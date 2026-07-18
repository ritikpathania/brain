use brain_core::errors::BrainError;
use brain_core::repositories::{
    ConfigRepository, EdgeRepository, EmbeddingRepository, NodeRepository, SessionRepository,
};
use brain_domain::{
    Edge, EdgeId, Embedding, Message, MessageId, MessageRole, Node, NodeId, NodeType, RelationId,
    RelationKind, Session, SessionId, SessionTimestamp, SessionTitle,
};
use brain_storage::{SqliteStorage, TestStorage};
use std::panic;

#[test]
fn test_sqlite_storage_node_crud() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let node_id = NodeId::new();
    let node = Node::new(node_id, "Person A".to_string(), NodeType::Person);

    NodeRepository::save(store, &node).unwrap();

    let fetched = NodeRepository::find_by_id(store, &node_id)
        .unwrap()
        .unwrap();
    assert_eq!(fetched.label, "Person A");
    assert_eq!(fetched.node_type, NodeType::Person);

    let node2_id = NodeId::new();
    let node2 = Node::new(node2_id, "Project X".to_string(), NodeType::Project);
    NodeRepository::save_batch(store, &[node2]).unwrap();

    let all = NodeRepository::list_all(store).unwrap();
    assert_eq!(all.len(), 2);

    NodeRepository::delete(store, &node_id).unwrap();
    assert!(NodeRepository::find_by_id(store, &node_id)
        .unwrap()
        .is_none());

    test_store.assert_clean();
}

#[test]
fn test_sqlite_storage_cascade_deletes() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let src_id = NodeId::new();
    let tgt_id = NodeId::new();
    let node_src = Node::new(src_id, "Src".to_string(), NodeType::Concept);
    let node_tgt = Node::new(tgt_id, "Tgt".to_string(), NodeType::Concept);

    NodeRepository::save(store, &node_src).unwrap();
    NodeRepository::save(store, &node_tgt).unwrap();

    let edge = Edge::new(src_id, tgt_id, RelationKind::AssociatedWith, 1.0);
    EdgeRepository::save(store, &edge).unwrap();

    let embedding = Embedding::new(src_id, vec![0.1, 0.2, 0.3]);
    EmbeddingRepository::save(store, &embedding).unwrap();

    let edge_id = EdgeId::new(src_id, tgt_id, RelationId::new("associated_with"));
    assert!(EdgeRepository::find_by_id(store, &edge_id)
        .unwrap()
        .is_some());
    assert!(EmbeddingRepository::find_by_node_id(store, &src_id)
        .unwrap()
        .is_some());

    // Delete source node
    NodeRepository::delete(store, &src_id).unwrap();

    // Check cascade deletes
    assert!(EdgeRepository::find_by_id(store, &edge_id)
        .unwrap()
        .is_none());
    assert!(EmbeddingRepository::find_by_node_id(store, &src_id)
        .unwrap()
        .is_none());

    test_store.assert_clean();
}

#[test]
fn test_sqlite_storage_transaction_rollback() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    // Let's test that saving an edge without source/target nodes fails
    let src_id = NodeId::new();
    let tgt_id = NodeId::new();
    let edge = Edge::new(src_id, tgt_id, RelationKind::AssociatedWith, 1.0);

    let save_res = EdgeRepository::save(store, &edge);
    assert!(save_res.is_err());

    // Attempt save_batch where the first edge is valid (existing nodes) and second edge is invalid.
    let valid_src = NodeId::new();
    let valid_tgt = NodeId::new();
    NodeRepository::save(
        store,
        &Node::new(valid_src, "V1".to_string(), NodeType::Concept),
    )
    .unwrap();
    NodeRepository::save(
        store,
        &Node::new(valid_tgt, "V2".to_string(), NodeType::Concept),
    )
    .unwrap();

    let valid_edge = Edge::new(valid_src, valid_tgt, RelationKind::AssociatedWith, 1.0);
    let invalid_edge = Edge::new(
        NodeId::new(),
        NodeId::new(),
        RelationKind::AssociatedWith,
        1.0,
    );

    let batch_res = EdgeRepository::save_batch(store, &[valid_edge.clone(), invalid_edge]);
    assert!(batch_res.is_err());

    // Confirm that the valid_edge was rolled back and does not exist in the database!
    let edge_id = EdgeId::new(valid_src, valid_tgt, RelationId::new("associated_with"));
    assert!(EdgeRepository::find_by_id(store, &edge_id)
        .unwrap()
        .is_none());

    test_store.assert_clean();
}

#[test]
fn test_sqlite_storage_migration_idempotence() {
    let mut temp_db = std::env::temp_dir();
    temp_db.push(format!("brain_test_{}.db", uuid::Uuid::new_v4()));
    let db_path = temp_db.to_str().unwrap().to_string();

    // 1. First initialization (runs migrations)
    {
        let _store = SqliteStorage::new(&db_path, 1, false).unwrap();
    }

    // 2. Second initialization (runs migrations again on the existing database)
    let second_run = SqliteStorage::new(&db_path, 1, false);
    assert!(
        second_run.is_ok(),
        "Second initialization should succeed without error"
    );

    // Clean up
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_sqlite_storage_session_and_config() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let session_id = SessionId::new();
    let mut session = Session::new(
        session_id,
        SessionTitle("test session".to_string()),
        SessionTimestamp(0),
    );
    session
        .add_message(Message::new(
            MessageId::new(),
            MessageRole::User,
            "Hello".to_string(),
        ))
        .unwrap();

    SessionRepository::save_session(store, &session_id, &session).unwrap();

    let loaded = SessionRepository::load_session(store, &session_id)
        .unwrap()
        .unwrap();
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "Hello");

    SessionRepository::delete_session(store, &session_id).unwrap();
    assert!(SessionRepository::load_session(store, &session_id)
        .unwrap()
        .is_none());

    ConfigRepository::save_key(store, "a", "1").unwrap();
    assert_eq!(ConfigRepository::get_key(store, "a").unwrap().unwrap(), "1");
    assert!(ConfigRepository::get_key(store, "b").unwrap().is_none());

    test_store.assert_clean();
}

#[test]
fn test_run_transaction_commit() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let node1_id = NodeId::new();
    let node2_id = NodeId::new();

    // Run transaction
    let result = store.run_transaction(|tx| {
        let nodes = tx.repositories().nodes();
        let edges = tx.repositories().edges();

        let n1 = Node::new(node1_id, "Node 1".to_string(), NodeType::Concept);
        let n2 = Node::new(node2_id, "Node 2".to_string(), NodeType::Concept);

        nodes.save(&n1)?;
        nodes.save(&n2)?;

        let edge = Edge::new(node1_id, node2_id, RelationKind::AssociatedWith, 1.0);
        edges.save(&edge)?;

        Ok("transaction success")
    });

    assert_eq!(result.unwrap(), "transaction success");

    // Assert that changes were saved successfully
    assert!(NodeRepository::find_by_id(store, &node1_id)
        .unwrap()
        .is_some());
    assert!(NodeRepository::find_by_id(store, &node2_id)
        .unwrap()
        .is_some());
    let edge_id = EdgeId::new(node1_id, node2_id, RelationId::new("associated_with"));
    assert!(EdgeRepository::find_by_id(store, &edge_id)
        .unwrap()
        .is_some());

    test_store.assert_clean();
}

#[test]
fn test_run_transaction_rollback() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let node1_id = NodeId::new();
    let node2_id = NodeId::new();

    // Run transaction that fails
    let result = store.run_transaction(|tx| {
        let nodes = tx.repositories().nodes();
        let edges = tx.repositories().edges();

        let n1 = Node::new(node1_id, "Node 1".to_string(), NodeType::Concept);
        nodes.save(&n1)?;

        // Try to save an edge where target node does not exist, which violates foreign keys and fails
        let edge = Edge::new(node1_id, node2_id, RelationKind::AssociatedWith, 1.0);
        edges.save(&edge)?;

        Ok("should fail")
    });

    assert!(result.is_err());

    // Assert that the transaction was rolled back and Node 1 was NOT saved
    assert!(NodeRepository::find_by_id(store, &node1_id)
        .unwrap()
        .is_none());

    test_store.assert_clean();
}

#[test]
fn test_nested_transaction_prevention() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let result = store.run_transaction(|_tx| {
        // Attempting to run a nested transaction using the same store
        let nested_result = store.run_transaction(|_nested_tx| Ok(()));

        assert!(nested_result.is_err());
        if let Err(brain_core::errors::BrainError::Storage { message, .. }) = nested_result {
            assert!(message.contains("Nested transactions are not supported"));
        } else {
            panic!("Expected a Storage error with nested transactions message");
        }

        Ok(())
    });

    assert!(result.is_ok());
    test_store.assert_clean();
}

#[test]
fn test_run_transaction_panic_safety() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let node1_id = NodeId::new();

    // Execute run_transaction, which panics inside the closure
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let _ = store.run_transaction(|tx| -> Result<(), BrainError> {
            let nodes = tx.repositories().nodes();
            let n1 = Node::new(node1_id, "Node 1".to_string(), NodeType::Concept);
            nodes.save(&n1).unwrap();

            panic!("Simulated panic inside transaction closure");
        });
    }));

    assert!(result.is_err(), "Closure execution should have panicked");

    // Assert that changes were rolled back (Node 1 does not exist)
    assert!(NodeRepository::find_by_id(store, &node1_id)
        .unwrap()
        .is_none());

    // Assert that connection pool returned to clean state (no leaks)
    test_store.assert_clean();
}

#[test]
fn test_sqlite_storage_node_conflict_merge() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let node_id = NodeId::new();

    // 1. Create a node with "stub" type (represented as Unknown)
    let node_stub = Node::new(node_id, "Test Node".to_string(), NodeType::Unknown).with_properties(
        std::collections::HashMap::from([("k1".to_string(), serde_json::json!("v1"))]),
    );
    NodeRepository::save(store, &node_stub).unwrap();

    // 2. Overwrite it with a real "Concept" type and additional properties
    let node_real = Node::new(node_id, "Test Node Real".to_string(), NodeType::Concept)
        .with_properties(std::collections::HashMap::from([
            ("k2".to_string(), serde_json::json!("v2")),
            ("k1".to_string(), serde_json::json!("v1-updated")),
        ]));
    NodeRepository::save(store, &node_real).unwrap();

    // 3. Fetch and verify:
    // - node_type should resolve to Concept (updating from stub)
    // - properties should merge (k1 updated, k2 added)
    let fetched = NodeRepository::find_by_id(store, &node_id)
        .unwrap()
        .unwrap();
    assert_eq!(fetched.node_type, NodeType::Concept);
    assert_eq!(fetched.label, "Test Node Real");
    assert_eq!(fetched.properties.get("k1").unwrap(), "v1-updated");
    assert_eq!(fetched.properties.get("k2").unwrap(), "v2");

    // 4. Overwrite it with Tool type to verify non-stub node_type is preserved
    let node_other =
        Node::new(node_id, "Test Node Other".to_string(), NodeType::Tool).with_properties(
            std::collections::HashMap::from([("k3".to_string(), serde_json::json!("v3"))]),
        );
    NodeRepository::save(store, &node_other).unwrap();

    let fetched2 = NodeRepository::find_by_id(store, &node_id)
        .unwrap()
        .unwrap();
    assert_eq!(fetched2.node_type, NodeType::Concept); // preserved
    assert_eq!(fetched2.properties.get("k1").unwrap(), "v1-updated");
    assert_eq!(fetched2.properties.get("k3").unwrap(), "v3");

    test_store.assert_clean();
}

#[test]
fn test_sqlite_edge_id_round_trip() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let node1_id = NodeId::new();
    let node2_id = NodeId::new();

    NodeRepository::save(
        store,
        &Node::new(node1_id, "Node 1".to_string(), NodeType::Concept),
    )
    .unwrap();
    NodeRepository::save(
        store,
        &Node::new(node2_id, "Node 2".to_string(), NodeType::Concept),
    )
    .unwrap();

    // 1. Create edge with a custom RelationId
    let relation_id = RelationId::new("configures");
    let edge_id = EdgeId::new(node1_id, node2_id, relation_id.clone());
    let edge = Edge::new(node1_id, node2_id, RelationKind::Configures, 0.75);

    // 2. Save edge to database
    EdgeRepository::save(store, &edge).unwrap();

    // 3. Find edge by EdgeId
    let fetched_edge = EdgeRepository::find_by_id(store, &edge_id)
        .unwrap()
        .expect("Edge not found");
    assert_eq!(fetched_edge.source, node1_id);
    assert_eq!(fetched_edge.target, node2_id);
    assert_eq!(fetched_edge.relation, RelationKind::Configures);

    // Verify that the relation ID derived from RelationKind matches the query key exactly
    assert_eq!(fetched_edge.relation.id(), relation_id);
    assert_eq!(fetched_edge.relation.id().as_str(), "configures");

    test_store.assert_clean();
}

#[test]
fn test_sqlite_learned_ranking_serialization() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    use brain_domain::identifiers::NodeId;
    use brain_domain::retrieval::models::{
        CalibrationMetadata, FeedbackEvent, RankingWeight, RankingWeights, SnapshotMetadata,
        SnapshotVersion, WeightSnapshot,
    };
    use brain_domain::temporal::TimePoint;

    // 1. Create and save a WeightSnapshot
    let metadata = SnapshotMetadata {
        version: SnapshotVersion::new(10),
        created_at: TimePoint::from_unix_seconds(1620000000),
        calibration_metadata: CalibrationMetadata::new("LinearAdjustment".to_string(), Some(0.005)),
    };
    let weights = RankingWeights::new(
        RankingWeight::new(0.5).unwrap(),
        RankingWeight::new(1.2).unwrap(),
        RankingWeight::new(2.1).unwrap(),
        RankingWeight::new(0.0).unwrap(),
    );
    let snapshot = WeightSnapshot { metadata, weights };

    store.save_weight_snapshot(&snapshot).unwrap();

    // 2. Fetch and assert
    let loaded = store
        .get_weight_snapshot(SnapshotVersion::new(10))
        .unwrap()
        .expect("Snapshot not found");
    assert_eq!(loaded.metadata.version.value(), 10);
    assert_eq!(loaded.metadata.created_at.unix_seconds(), 1620000000);
    assert_eq!(
        loaded.metadata.calibration_metadata.algorithm_used(),
        "LinearAdjustment"
    );
    assert_eq!(
        loaded.metadata.calibration_metadata.validation_loss(),
        Some(0.005)
    );
    assert_eq!(loaded.weights.semantic().value(), 0.5);
    assert_eq!(loaded.weights.graph().value(), 1.2);
    assert_eq!(loaded.weights.recency().value(), 2.1);
    assert_eq!(loaded.weights.temporal().value(), 0.0);

    // List all
    let all_snapshots = store.list_all_weight_snapshots().unwrap();
    // Default version 1 exists from migration + our version 10
    assert_eq!(all_snapshots.len(), 2);
    assert_eq!(all_snapshots[0].metadata.version.value(), 1);
    assert_eq!(all_snapshots[1].metadata.version.value(), 10);

    // 3. Create and save a FeedbackEvent
    let event = FeedbackEvent {
        id: "evt-999".to_string(),
        schema_version: 1,
        query: "rust memory".to_string(),
        node_id: NodeId::new(),
        selected: true,
        timestamp: 1620000005,
        ranking_position: 3,
        context: "{\"session\":\"abc\"}".to_string(),
    };

    store.save_feedback_event(&event).unwrap();

    let all_events = store.list_all_feedback_events().unwrap();
    assert_eq!(all_events.len(), 1);
    let loaded_evt = &all_events[0];
    assert_eq!(loaded_evt.id, "evt-999");
    assert_eq!(loaded_evt.schema_version, 1);
    assert_eq!(loaded_evt.query, "rust memory");
    assert_eq!(loaded_evt.node_id, event.node_id);
    assert!(loaded_evt.selected);
    assert_eq!(loaded_evt.timestamp, 1620000005);
    assert_eq!(loaded_evt.ranking_position, 3);
    assert_eq!(loaded_evt.context, "{\"session\":\"abc\"}");

    test_store.assert_clean();
}

#[test]
fn test_ivf_vector_indexing() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let node1_id = NodeId::new();
    let node2_id = NodeId::new();

    // 1. Create a node for each embedding to satisfy foreign key constraints
    let n1 = Node::new(node1_id, "Node 1".to_string(), NodeType::Concept);
    let n2 = Node::new(node2_id, "Node 2".to_string(), NodeType::Concept);
    NodeRepository::save_batch(store, &[n1, n2]).unwrap();

    // 2. Generate vectors that align perfectly with Centroid 0 and Centroid 4
    // Centroid sinusoidal pattern for index c:
    // v_c[i] = sin(2*pi * (i+1) * (c+1) / 384)
    let mut vec1 = vec![0.0f32; 384];
    let mut vec2 = vec![0.0f32; 384];
    for i in 0..384 {
        vec1[i] = ((2.0 * std::f64::consts::PI * (i + 1) as f64 * 1.0) / 384.0).sin() as f32;
        vec2[i] = ((2.0 * std::f64::consts::PI * (i + 1) as f64 * 5.0) / 384.0).sin() as f32;
    }

    let emb1 = Embedding::new(node1_id, vec1);
    let emb2 = Embedding::new(node2_id, vec2);

    // 3. Save the embeddings
    EmbeddingRepository::save(store, &emb1).unwrap();
    EmbeddingRepository::save(store, &emb2).unwrap();

    // 4. Verify find_by_node_id works
    let loaded1 = EmbeddingRepository::find_by_node_id(store, &node1_id)
        .unwrap()
        .unwrap();
    let loaded2 = EmbeddingRepository::find_by_node_id(store, &node2_id)
        .unwrap()
        .unwrap();
    assert_eq!(loaded1.node_id, node1_id);
    assert_eq!(loaded2.node_id, node2_id);

    // 5. Query by centroids
    // Node 1 should be mapped to Centroid 0.
    let centroid0_results = EmbeddingRepository::find_by_centroids(store, &[0]).unwrap();
    assert_eq!(centroid0_results.len(), 1);
    assert_eq!(centroid0_results[0].node_id, node1_id);

    // Node 2 should be mapped to Centroid 4.
    let centroid4_results = EmbeddingRepository::find_by_centroids(store, &[4]).unwrap();
    assert_eq!(centroid4_results.len(), 1);
    assert_eq!(centroid4_results[0].node_id, node2_id);

    // Querying both centroids should return both embeddings.
    let combined_results = EmbeddingRepository::find_by_centroids(store, &[0, 4]).unwrap();
    assert_eq!(combined_results.len(), 2);
    let ids: Vec<NodeId> = combined_results.iter().map(|e| e.node_id).collect();
    assert!(ids.contains(&node1_id));
    assert!(ids.contains(&node2_id));

    // Querying non-matching centroids should return empty.
    let empty_results = EmbeddingRepository::find_by_centroids(store, &[2, 7]).unwrap();
    assert!(empty_results.is_empty());
}
