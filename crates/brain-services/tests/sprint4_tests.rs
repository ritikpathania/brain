//! Sprint 4 runtime validation tests.

#[cfg(test)]
mod tests {
    use brain_core::{
        events::{CorrelationId, RuntimeEventDispatcher, RuntimeRelationshipEvent},
        evolution::{Canonicalizer, Observation, Provenance},
        repositories::Storage,
    };
    use brain_domain::{Edge, Node, NodeId, NodeKind, RelationKind};
    use brain_services::{InMemoryEventDispatcher, SqliteCanonicalizer, SqliteReflectionEngine};
    use brain_storage::test_utils::TestStorage;
    use std::sync::Arc;
    use std::time::SystemTime;

    fn make_obs(payload: &str, corr_id: CorrelationId) -> Observation {
        Observation {
            payload: payload.as_bytes().to_vec(),
            media_type: "text/plain".to_string(),
            provenance: Provenance {
                source_adapter: "test".to_string(),
                timestamp: SystemTime::now(),
                correlation_id: corr_id,
            },
        }
    }

    fn deterministic_node_id(payload: &str) -> NodeId {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        payload.hash(&mut hasher);
        let hash_val = hasher.finish();
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&hash_val.to_be_bytes());
        bytes[8..16].copy_from_slice(&hash_val.to_be_bytes());
        NodeId(uuid::Uuid::from_bytes(bytes))
    }

    // --- S4-RT-1: Edge strengthened after canonicalization ---
    //
    // Verifies that adjacent edge weight increases after canonicalization of the node.
    #[test]
    fn test_edge_strengthened_after_canonicalization() {
        let test_db = TestStorage::new();
        let storage: Arc<dyn Storage> = Arc::new(test_db.storage().clone());
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(64));
        let dispatcher_trait: Arc<dyn RuntimeEventDispatcher> =
            Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;

        let reflection_engine = Arc::new(SqliteReflectionEngine::new(
            Arc::clone(&storage),
            Arc::clone(&dispatcher_trait),
        ));

        let canonicalizer =
            SqliteCanonicalizer::new(Arc::clone(&storage), Arc::clone(&dispatcher_trait))
                .with_reflection(reflection_engine);

        let payload_a = "Concept A";
        let payload_b = "Concept B";
        let node_a_id = deterministic_node_id(payload_a);
        let node_b_id = deterministic_node_id(payload_b);

        let node_a = Node::new(node_a_id, payload_a.to_string(), NodeKind::Concept);
        let node_b = Node::new(node_b_id, payload_b.to_string(), NodeKind::Concept);

        // communicates_via has confidence_strategy: average
        let edge = Edge::new(node_a_id, node_b_id, RelationKind::CommunicatesVia, 0.4);

        // Pre-save to storage
        test_db
            .storage()
            .run_transaction(|tx| {
                let repos = tx.repositories();
                repos.nodes().save(&node_a)?;
                repos.nodes().save(&node_b)?;
                repos.edges().save(&edge)?;
                Ok(())
            })
            .unwrap();

        let corr_id = CorrelationId::new_v4();
        let obs = make_obs(payload_a, corr_id);

        let result = canonicalizer.canonicalize(obs).unwrap();
        assert_eq!(result.epoch.0, 1);
        assert_eq!(result.affected_entities[0], node_a_id);

        // Query storage to verify updated weight
        let updated_edge = test_db
            .storage()
            .run_transaction(|tx| {
                let repos = tx.repositories();
                repos.edges().find_by_id(&brain_domain::EdgeId::new(
                    node_a_id,
                    node_b_id,
                    RelationKind::CommunicatesVia.id(),
                ))
            })
            .unwrap()
            .expect("Edge should exist");

        // (0.4 + 1.0) / 2.0 = 0.7
        assert_eq!(updated_edge.weight, 0.7);
    }

    // --- S4-RT-2: RuntimeRelationshipEvent dispatched to subscriber ---
    //
    // Verifies that a subscriber receives the RuntimeRelationshipEvent.
    #[test]
    fn test_relationship_strengthened_event_dispatched() {
        let test_db = TestStorage::new();
        let storage: Arc<dyn Storage> = Arc::new(test_db.storage().clone());
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(64));
        let dispatcher_trait: Arc<dyn RuntimeEventDispatcher> =
            Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;

        let reflection_engine = Arc::new(SqliteReflectionEngine::new(
            Arc::clone(&storage),
            Arc::clone(&dispatcher_trait),
        ));

        let canonicalizer =
            SqliteCanonicalizer::new(Arc::clone(&storage), Arc::clone(&dispatcher_trait))
                .with_reflection(reflection_engine);

        let payload_a = "Concept A";
        let payload_b = "Concept B";
        let node_a_id = deterministic_node_id(payload_a);
        let node_b_id = deterministic_node_id(payload_b);

        let node_a = Node::new(node_a_id, payload_a.to_string(), NodeKind::Concept);
        let node_b = Node::new(node_b_id, payload_b.to_string(), NodeKind::Concept);
        let edge = Edge::new(node_a_id, node_b_id, RelationKind::CommunicatesVia, 0.4);

        test_db
            .storage()
            .run_transaction(|tx| {
                let repos = tx.repositories();
                repos.nodes().save(&node_a)?;
                repos.nodes().save(&node_b)?;
                repos.edges().save(&edge)?;
                Ok(())
            })
            .unwrap();

        let mut rx = dispatcher.subscribe();

        let corr_id = CorrelationId::new_v4();
        let obs = make_obs(payload_a, corr_id);
        let _result = canonicalizer.canonicalize(obs).unwrap();

        // Drain channel to find RuntimeRelationshipEvent
        let mut found_relationship_event = false;
        while let Ok(event) = rx.try_recv() {
            if let Some(rel_ev) = event.as_any().downcast_ref::<RuntimeRelationshipEvent>() {
                assert_eq!(rel_ev.correlation_id, corr_id);
                if let brain_domain::events::DomainEvent::RelationshipStrengthened {
                    source,
                    target,
                    relation,
                    new_weight,
                } = &rel_ev.domain_event
                {
                    assert_eq!(source, &node_a_id.to_string());
                    assert_eq!(target, &node_b_id.to_string());
                    assert_eq!(relation, "communicates_via");
                    assert_eq!(*new_weight, 0.7);
                    found_relationship_event = true;
                }
            }
        }
        assert!(
            found_relationship_event,
            "Expected RuntimeRelationshipEvent to be dispatched"
        );
    }

    // --- S4-RT-3: ConfidenceStrategy applied dynamically ---
    //
    // Verifies that Maximum, Average, and SourceDefined strategies are applied correctly.
    #[test]
    fn test_confidence_strategy_applied() {
        let test_db = TestStorage::new();
        let storage: Arc<dyn Storage> = Arc::new(test_db.storage().clone());
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(64));
        let dispatcher_trait: Arc<dyn RuntimeEventDispatcher> =
            Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;

        let reflection_engine = Arc::new(SqliteReflectionEngine::new(
            Arc::clone(&storage),
            Arc::clone(&dispatcher_trait),
        ));

        let canonicalizer =
            SqliteCanonicalizer::new(Arc::clone(&storage), Arc::clone(&dispatcher_trait))
                .with_reflection(reflection_engine);

        let payload_a = "Concept A";
        let payload_b = "Concept B";
        let node_a_id = deterministic_node_id(payload_a);
        let node_b_id = deterministic_node_id(payload_b);

        let node_a = Node::new(node_a_id, payload_a.to_string(), NodeKind::Concept);
        let node_b = Node::new(node_b_id, payload_b.to_string(), NodeKind::Concept);

        // Create 3 edges with different RelationKinds:
        // 1. uses: confidence_strategy = maximum
        let edge_uses = Edge::new(node_a_id, node_b_id, RelationKind::Uses, 0.5);
        // 2. communicates_via: confidence_strategy = average
        let edge_comm = Edge::new(node_a_id, node_b_id, RelationKind::CommunicatesVia, 0.4);
        // 3. runs_on: confidence_strategy = source_defined
        let edge_runs = Edge::new(node_a_id, node_b_id, RelationKind::RunsOn, 0.8);

        test_db
            .storage()
            .run_transaction(|tx| {
                let repos = tx.repositories();
                repos.nodes().save(&node_a)?;
                repos.nodes().save(&node_b)?;
                repos.edges().save(&edge_uses)?;
                repos.edges().save(&edge_comm)?;
                repos.edges().save(&edge_runs)?;
                Ok(())
            })
            .unwrap();

        let corr_id = CorrelationId::new_v4();
        let obs = make_obs(payload_a, corr_id);
        let _result = canonicalizer.canonicalize(obs).unwrap();

        test_db
            .storage()
            .run_transaction(|tx| {
                let repos = tx.repositories();

                let u_uses = repos
                    .edges()
                    .find_by_id(&brain_domain::EdgeId::new(
                        node_a_id,
                        node_b_id,
                        RelationKind::Uses.id(),
                    ))?
                    .unwrap();
                // max(0.5, 1.0) = 1.0
                assert_eq!(u_uses.weight, 1.0);

                let u_comm = repos
                    .edges()
                    .find_by_id(&brain_domain::EdgeId::new(
                        node_a_id,
                        node_b_id,
                        RelationKind::CommunicatesVia.id(),
                    ))?
                    .unwrap();
                // average(0.4, 1.0) = 0.7
                assert_eq!(u_comm.weight, 0.7);

                let u_runs = repos
                    .edges()
                    .find_by_id(&brain_domain::EdgeId::new(
                        node_a_id,
                        node_b_id,
                        RelationKind::RunsOn.id(),
                    ))?
                    .unwrap();
                // source_defined(0.8, 1.0) = 0.8 * 1.0 = 0.8
                assert_eq!(u_runs.weight, 0.8);

                Ok(())
            })
            .unwrap();
    }

    // --- S4-RT-4: Reflection edge transaction atomicity ---
    //
    // Verifies that if reflection fails midway, all edge updates are rolled back.
    #[test]
    fn test_reflection_edge_transaction_atomicity() {
        let test_db = TestStorage::new();
        let storage: Arc<dyn Storage> = Arc::new(test_db.storage().clone());
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(64));
        let dispatcher_trait: Arc<dyn RuntimeEventDispatcher> =
            Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;

        let reflection_engine = Arc::new(SqliteReflectionEngine::new(
            Arc::clone(&storage),
            Arc::clone(&dispatcher_trait),
        ));

        let canonicalizer =
            SqliteCanonicalizer::new(Arc::clone(&storage), Arc::clone(&dispatcher_trait))
                .with_reflection(reflection_engine);

        let payload_a = "Concept A";
        let payload_b = "Concept B";
        let node_a_id = deterministic_node_id(payload_a);
        let node_b_id = deterministic_node_id(payload_b);

        let node_a = Node::new(node_a_id, payload_a.to_string(), NodeKind::Concept);
        let node_b = Node::new(node_b_id, payload_b.to_string(), NodeKind::Concept);

        // Pre-insert an edge with valid weight first, but wait:
        // We will manually construct an edge with invalid weight in DB so that strengthen fails.
        let mut invalid_edge = Edge::new(node_a_id, node_b_id, RelationKind::CommunicatesVia, 0.4);
        invalid_edge.weight = 2.5; // Invalid weight (> 1.0) to trigger DomainError

        test_db
            .storage()
            .run_transaction(|tx| {
                let repos = tx.repositories();
                repos.nodes().save(&node_a)?;
                repos.nodes().save(&node_b)?;
                repos.edges().save(&invalid_edge)?;
                Ok(())
            })
            .unwrap();

        // Run canonicalization. This will attempt to reflect on node_a, find the invalid_edge,
        // call strengthen_with_evidence which returns Err(InvalidEdgeWeight), causing the
        // reflection engine transaction to abort and return an error.
        // The error is swallowed/non-fatal in SqliteCanonicalizer, but the transaction rolls back.
        let corr_id = CorrelationId::new_v4();
        let obs = make_obs(payload_a, corr_id);
        let result = canonicalizer.canonicalize(obs).unwrap();
        assert_eq!(result.epoch.0, 1);

        // Verify the database edge weight is still 2.5 (has not been mutated/updated to something else)
        let final_edge = test_db
            .storage()
            .run_transaction(|tx| {
                let repos = tx.repositories();
                repos.edges().find_by_id(&brain_domain::EdgeId::new(
                    node_a_id,
                    node_b_id,
                    RelationKind::CommunicatesVia.id(),
                ))
            })
            .unwrap()
            .unwrap();

        assert_eq!(final_edge.weight, 2.5);
    }
}
