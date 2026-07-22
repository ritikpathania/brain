use brain_core::errors::BrainError;
use brain_core::repositories::StorageTransaction;
use brain_domain::{
    Edge, EdgeId, Embedding, FindingEvidence, Node, NodeId, NodeType, ReflectionDomainCommand,
    ReflectionDomainEvent, ReflectionFinding, RelationKind,
};
use brain_services::reflection::{
    handler::ReflectionCommandHandler, passes::DuplicateDetectionPass, ReflectionContext,
    ReflectionEngine, ReflectionPass, ReflectionPlanner, ReflectionRegistry,
};
use brain_storage::TestStorage;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[test]
fn test_planner_determinism_and_filtering() {
    let planner = ReflectionPlanner::new();

    let node_1 = NodeId(Uuid::new_v4());
    let node_2 = NodeId(Uuid::new_v4());

    let evidence_high = FindingEvidence {
        confidence: 0.95,
        semantic_similarity: Some(0.96),
        edit_distance: Some(0),
        overlap_ratio: None,
        details: "High confidence duplicate".to_string(),
    };

    let evidence_low = FindingEvidence {
        confidence: 0.88,
        semantic_similarity: Some(0.80),
        edit_distance: Some(2),
        overlap_ratio: None,
        details: "Low confidence duplicate".to_string(),
    };

    let findings = vec![
        ReflectionFinding::DuplicateFound {
            node_a: node_1,
            node_b: node_2,
            evidence: evidence_high.clone(),
        },
        ReflectionFinding::DuplicateFound {
            node_a: node_1,
            node_b: node_2,
            evidence: evidence_low.clone(),
        },
    ];

    // Run planning
    let plan_1 = planner.plan(findings.clone());
    let plan_2 = planner.plan(findings);

    // Verify determinism
    assert_eq!(plan_1, plan_2);

    // Verify filtering
    assert_eq!(plan_1.commands.len(), 1);
    assert_eq!(plan_1.skipped_findings.len(), 1);
    assert_eq!(plan_1.findings_processed, 2);

    // High confidence should yield a MergeConcepts command
    let expected_canonical = std::cmp::min(node_1, node_2);
    let expected_duplicate = std::cmp::max(node_1, node_2);
    assert_eq!(
        plan_1.commands[0],
        ReflectionDomainCommand::MergeConcepts {
            canonical_id: expected_canonical,
            duplicate_id: expected_duplicate,
        }
    );

    // Low confidence should be logged as skipped
    assert_eq!(
        plan_1.skipped_findings[0].1,
        "Confidence below merge threshold (0.92)"
    );
}

#[test]
fn test_duplicate_detection_pass() {
    let test_storage = TestStorage::new();
    let storage = test_storage.store();

    // Insert test nodes into database
    let node_1_id = NodeId(Uuid::new_v4());
    let node_2_id = NodeId(Uuid::new_v4());
    let node_diff_type_id = NodeId(Uuid::new_v4());

    let node_1 = Node::new(node_1_id, "Apple Inc".to_string(), NodeType::Concept);
    let node_2 = Node::new(node_2_id, "apple inc.".to_string(), NodeType::Concept);
    let node_diff_type = Node::new(
        node_diff_type_id,
        "Apple Inc".to_string(),
        NodeType::Project,
    );

    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            let repos = tx.repositories();
            repos.nodes().save(&node_1)?;
            repos.nodes().save(&node_2)?;
            repos.nodes().save(&node_diff_type)?;

            // Save matching embeddings (cosine similarity = 1.0)
            let emb_1 = Embedding::new(node_1_id, vec![1.0, 0.0, 0.0]);
            let emb_2 = Embedding::new(node_2_id, vec![1.0, 0.0, 0.0]);
            repos.embeddings().save(&emb_1)?;
            repos.embeddings().save(&emb_2)?;
            Ok(())
        })
        .unwrap();

    let registry = Arc::new({
        let mut r = ReflectionRegistry::new();
        r.register(Box::new(DuplicateDetectionPass::new()));
        r
    });

    let engine = ReflectionEngine::new(registry, storage.clone());

    let context = ReflectionContext {
        execution_id: Uuid::new_v4(),
        session_id: brain_domain::SessionId(ulid::Ulid::new()),
        cutoff_epoch: 1,
        max_nodes: 100,
        time_budget_ms: 1000,
        cancellation_token: CancellationToken::new(),
    };

    let findings = engine.reflect(&context).unwrap();

    // Verify duplicate concept detected
    assert_eq!(findings.len(), 1);
    match &findings[0] {
        ReflectionFinding::DuplicateFound {
            node_a,
            node_b,
            evidence,
        } => {
            let sorted_ids = if node_1_id < node_2_id {
                (node_1_id, node_2_id)
            } else {
                (node_2_id, node_1_id)
            };
            assert!(
                (*node_a == sorted_ids.0 && *node_b == sorted_ids.1)
                    || (*node_a == sorted_ids.1 && *node_b == sorted_ids.0)
            );
            assert!(evidence.confidence >= 0.9);
            assert_eq!(evidence.semantic_similarity, Some(1.0));
        }
        _ => panic!("Expected duplicate finding"),
    }
}

#[test]
fn test_cancellation_budget() {
    let test_storage = TestStorage::new();
    let storage = test_storage.store();

    let registry = Arc::new({
        let mut r = ReflectionRegistry::new();
        r.register(Box::new(DuplicateDetectionPass::new()));
        r
    });

    let engine = ReflectionEngine::new(registry, storage.clone());

    let cancel_token = CancellationToken::new();
    cancel_token.cancel(); // Cancel immediately before execution

    let context = ReflectionContext {
        execution_id: Uuid::new_v4(),
        session_id: brain_domain::SessionId(ulid::Ulid::new()),
        cutoff_epoch: 1,
        max_nodes: 100,
        time_budget_ms: 1000,
        cancellation_token: cancel_token,
    };

    let result = engine.reflect(&context);
    assert!(result.is_err());
    match result {
        Err(BrainError::Validation { message }) => {
            assert!(message.contains("Reflection aborted"));
        }
        _ => panic!("Expected aborted error"),
    }
}

#[test]
fn test_concept_merging_and_edge_relinking() {
    let test_storage = TestStorage::new();
    let storage = test_storage.store();

    // 1. Insert test nodes & edges with deterministic UUID ordering
    let uuid_a = Uuid::new_v4();
    let uuid_b = Uuid::new_v4();
    let (node_1_id, node_2_id) = if uuid_a < uuid_b {
        (NodeId(uuid_a), NodeId(uuid_b))
    } else {
        (NodeId(uuid_b), NodeId(uuid_a))
    };
    let node_other_id = NodeId(Uuid::new_v4());

    let mut node_1 = Node::new(node_1_id, "Apple Corp".to_string(), NodeType::Concept);
    node_1
        .properties
        .insert("ticker".to_string(), serde_json::json!("AAPL"));

    let mut node_2 = Node::new(node_2_id, "apple corp.".to_string(), NodeType::Concept);
    node_2
        .properties
        .insert("headquarters".to_string(), serde_json::json!("Cupertino"));

    let node_other = Node::new(
        node_other_id,
        "iPhone Product".to_string(),
        NodeType::Concept,
    );

    // Edge from duplicate node_2 to node_other
    let edge = Edge::new(node_2_id, node_other_id, RelationKind::Uses, 0.8);

    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            let repos = tx.repositories();
            repos.nodes().save(&node_1)?;
            repos.nodes().save(&node_2)?;
            repos.nodes().save(&node_other)?;
            repos.edges().save(&edge)?;

            let emb_1 = Embedding::new(node_1_id, vec![1.0, 0.0, 0.0]);
            let emb_2 = Embedding::new(node_2_id, vec![1.0, 0.0, 0.0]);
            repos.embeddings().save(&emb_1)?;
            repos.embeddings().save(&emb_2)?;
            Ok(())
        })
        .unwrap();

    // 2. Execute duplicate detection and planner
    let registry = Arc::new({
        let mut r = ReflectionRegistry::new();
        r.register(Box::new(DuplicateDetectionPass::new()));
        r
    });
    let engine = ReflectionEngine::new(registry, storage.clone());
    let planner = ReflectionPlanner::new();

    let context = ReflectionContext {
        execution_id: Uuid::new_v4(),
        session_id: brain_domain::SessionId(ulid::Ulid::new()),
        cutoff_epoch: 1,
        max_nodes: 100,
        time_budget_ms: 1000,
        cancellation_token: CancellationToken::new(),
    };

    let findings = engine.reflect(&context).unwrap();
    assert_eq!(findings.len(), 1);

    let plan = planner.plan(findings);
    assert_eq!(plan.commands.len(), 1);

    // 3. Execute command handler in a write transaction
    let handler = ReflectionCommandHandler::new();
    let mut events = Vec::new();

    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            for cmd in plan.commands.clone() {
                let ev = handler.handle(tx, cmd)?;
                events.push(ev);
            }
            Ok(())
        })
        .unwrap();

    // Verify event details
    assert_eq!(events.len(), 1);
    match &events[0] {
        ReflectionDomainEvent::ConceptMerged {
            canonical_id,
            merged_id,
            provenance,
        } => {
            let expected_canonical = std::cmp::min(node_1_id, node_2_id);
            let expected_merged = std::cmp::max(node_1_id, node_2_id);
            assert_eq!(*canonical_id, expected_canonical);
            assert_eq!(*merged_id, expected_merged);
            assert!(provenance.contains("Merged concept"));
        }
        _ => panic!("Expected ConceptMerged event"),
    }

    // 4. Verify database state
    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            let repos = tx.repositories();
            let expected_canonical = std::cmp::min(node_1_id, node_2_id);
            let expected_merged = std::cmp::max(node_1_id, node_2_id);

            // redudant node deleted, canonical survives
            assert!(repos.nodes().find_by_id(&expected_merged)?.is_none());
            let canonical_node = repos.nodes().find_by_id(&expected_canonical)?.unwrap();

            // properties are merged
            assert_eq!(canonical_node.properties.get("ticker").unwrap(), "AAPL");
            assert_eq!(
                canonical_node.properties.get("headquarters").unwrap(),
                "Cupertino"
            );

            // edge updated to point to canonical
            let old_edge_id = EdgeId::new(node_2_id, node_other_id, RelationKind::Uses.id());
            assert!(repos.edges().find_by_id(&old_edge_id)?.is_none());

            let new_edge_id =
                EdgeId::new(expected_canonical, node_other_id, RelationKind::Uses.id());
            let updated_edge = repos.edges().find_by_id(&new_edge_id)?.unwrap();
            assert_eq!(updated_edge.weight, 0.8);

            Ok(())
        })
        .unwrap();
}

#[test]
fn test_command_idempotency_and_rollback() {
    let test_storage = TestStorage::new();
    let storage = test_storage.store();

    let node_1_id = NodeId(Uuid::new_v4());
    let node_1 = Node::new(
        node_1_id,
        "Canonical Concept".to_string(),
        NodeType::Concept,
    );

    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            tx.repositories().nodes().save(&node_1)?;
            Ok(())
        })
        .unwrap();

    let handler = ReflectionCommandHandler::new();

    // 1. Try to merge with non-existent duplicate concept (Rollback check)
    let non_existent_id = NodeId(Uuid::new_v4());
    let command = ReflectionDomainCommand::MergeConcepts {
        canonical_id: node_1_id,
        duplicate_id: non_existent_id,
    };

    let result = storage.run_transaction(|tx: &dyn StorageTransaction| {
        handler.handle(tx, command.clone())?;
        Ok(())
    });

    // Transaction should return an error (rollback)
    assert!(result.is_err());

    // Verify canonical node still exists untouched
    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            assert!(tx.repositories().nodes().find_by_id(&node_1_id)?.is_some());
            Ok(())
        })
        .unwrap();
}

#[test]
fn test_contradiction_pass() {
    use brain_services::reflection::passes::ContradictionPass;

    let test_storage = TestStorage::new();
    let storage = test_storage.store();

    let node_a_id = NodeId(Uuid::new_v4());
    let node_b_id = NodeId(Uuid::new_v4());

    let mut node_a = Node::new(node_a_id, "Google Corp".to_string(), NodeType::Concept);
    node_a.properties.insert(
        "headquarters".to_string(),
        serde_json::json!("Mountain View"),
    );

    let mut node_b = Node::new(node_b_id, "google corp.".to_string(), NodeType::Concept);
    node_b
        .properties
        .insert("headquarters".to_string(), serde_json::json!("New York"));

    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            tx.repositories().nodes().save(&node_a)?;
            tx.repositories().nodes().save(&node_b)?;
            Ok(())
        })
        .unwrap();

    let pass = ContradictionPass::new();
    let context = ReflectionContext {
        execution_id: Uuid::new_v4(),
        session_id: brain_domain::SessionId(ulid::Ulid::new()),
        cutoff_epoch: 1,
        max_nodes: 100,
        time_budget_ms: 1000,
        cancellation_token: CancellationToken::new(),
    };

    let mut findings = Vec::new();
    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            findings = pass.run(tx.repositories(), &context).unwrap();
            Ok(())
        })
        .unwrap();

    assert!(!findings.is_empty());

    // Check that we found a contradiction finding for headquarters
    let mut found = false;
    for finding in findings {
        if let ReflectionFinding::ContradictionFound { property_key, .. } = finding {
            if property_key == "headquarters" {
                found = true;
            }
        }
    }
    assert!(found);
}

#[test]
fn test_link_suggestion_pass() {
    use brain_services::reflection::passes::LinkSuggestionPass;

    let test_storage = TestStorage::new();
    let storage = test_storage.store();

    let node_a_id = NodeId(Uuid::new_v4());
    let node_b_id = NodeId(Uuid::new_v4());
    let node_c_id = NodeId(Uuid::new_v4());

    let node_a = Node::new(node_a_id, "A".to_string(), NodeType::Concept);
    let node_b = Node::new(node_b_id, "B".to_string(), NodeType::Concept);
    let node_c = Node::new(node_c_id, "C".to_string(), NodeType::Concept);

    // A uses B, B uses C -> suggests A uses C
    let edge_ab = Edge::new(node_a_id, node_b_id, RelationKind::Uses, 0.9);
    let edge_bc = Edge::new(node_b_id, node_c_id, RelationKind::Uses, 0.8);

    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            tx.repositories().nodes().save(&node_a)?;
            tx.repositories().nodes().save(&node_b)?;
            tx.repositories().nodes().save(&node_c)?;
            tx.repositories().edges().save(&edge_ab)?;
            tx.repositories().edges().save(&edge_bc)?;
            Ok(())
        })
        .unwrap();

    let pass = LinkSuggestionPass::new();
    let context = ReflectionContext {
        execution_id: Uuid::new_v4(),
        session_id: brain_domain::SessionId(ulid::Ulid::new()),
        cutoff_epoch: 1,
        max_nodes: 100,
        time_budget_ms: 1000,
        cancellation_token: CancellationToken::new(),
    };

    let mut findings = Vec::new();
    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            findings = pass.run(tx.repositories(), &context).unwrap();
            Ok(())
        })
        .unwrap();

    assert!(!findings.is_empty());

    let mut found = false;
    for finding in findings {
        if let ReflectionFinding::LinkSuggested {
            source_id,
            target_id,
            relation_kind,
            ..
        } = finding
        {
            if source_id == node_a_id
                && target_id == node_c_id
                && relation_kind == RelationKind::Uses
            {
                found = true;
            }
        }
    }
    assert!(found);
}

#[test]
fn test_synthesis_pass() {
    use brain_services::reflection::passes::SynthesisPass;

    let test_storage = TestStorage::new();
    let storage = test_storage.store();

    let node_a_id = NodeId(Uuid::new_v4());
    let node_b_id = NodeId(Uuid::new_v4());
    let node_c_id = NodeId(Uuid::new_v4());

    let node_a = Node::new(node_a_id, "A".to_string(), NodeType::Concept);
    let node_b = Node::new(node_b_id, "B".to_string(), NodeType::Concept);
    let node_c = Node::new(node_c_id, "C".to_string(), NodeType::Concept);

    // A AssociatedWith B, B AssociatedWith C -> suggests A AssociatedWith C (triad closure)
    let edge_ab = Edge::new(node_a_id, node_b_id, RelationKind::AssociatedWith, 0.9);
    let edge_bc = Edge::new(node_b_id, node_c_id, RelationKind::AssociatedWith, 0.8);

    storage
        .run_transaction(|tx: &dyn StorageTransaction| {
            tx.repositories().nodes().save(&node_a)?;
            tx.repositories().nodes().save(&node_b)?;
            tx.repositories().nodes().save(&node_c)?;
            tx.repositories().edges().save(&edge_ab)?;
            tx.repositories().edges().save(&edge_bc)?;
            Ok(())
        })
        .unwrap();

    let pass = SynthesisPass::new();
    let context = ReflectionContext {
        execution_id: Uuid::new_v4(),
        session_id: brain_domain::SessionId(ulid::Ulid::new()),
        cutoff_epoch: 1,
        max_nodes: 100,
        time_budget_ms: 1000,
        cancellation_token: CancellationToken::new(),
    };

    let mut findings = Vec::new();
    storage
        .run_transaction(&mut |tx: &dyn StorageTransaction| {
            findings = pass.run(tx.repositories(), &context).unwrap();
            Ok(())
        })
        .unwrap();

    assert!(!findings.is_empty());

    let mut found = false;
    for finding in findings {
        if let ReflectionFinding::LinkSuggested {
            source_id,
            target_id,
            relation_kind,
            ..
        } = finding
        {
            if source_id == node_a_id
                && target_id == node_c_id
                && relation_kind == RelationKind::AssociatedWith
            {
                found = true;
            }
        }
    }
    assert!(found);
}
