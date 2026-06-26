use brain_core::errors::BrainError;
use brain_core::repositories::{
    ConfigRepository, EdgeRepository, EmbeddingRepository, NodeRepository, SessionRepository,
};
use brain_domain::{
    Conversation, ConversationId, Edge, EdgeId, Embedding, Message, MessageId, MessageRole, Node,
    NodeId, NodeType, SessionId,
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

    let edge = Edge::new(src_id, tgt_id, "knows".to_string(), 1.0);
    EdgeRepository::save(store, &edge).unwrap();

    let embedding = Embedding::new(src_id, vec![0.1, 0.2, 0.3]);
    EmbeddingRepository::save(store, &embedding).unwrap();

    let edge_id = EdgeId::new(src_id, tgt_id, "knows".to_string());
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
    let edge = Edge::new(src_id, tgt_id, "knows".to_string(), 1.0);

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

    let valid_edge = Edge::new(valid_src, valid_tgt, "knows".to_string(), 1.0);
    let invalid_edge = Edge::new(NodeId::new(), NodeId::new(), "knows".to_string(), 1.0);

    let batch_res = EdgeRepository::save_batch(store, &[valid_edge.clone(), invalid_edge]);
    assert!(batch_res.is_err());

    // Confirm that the valid_edge was rolled back and does not exist in the database!
    let edge_id = EdgeId::new(valid_src, valid_tgt, "knows".to_string());
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
    let conversation = Conversation::new(ConversationId::new()).with_messages(vec![Message::new(
        MessageId::new(),
        MessageRole::User,
        "Hello".to_string(),
    )]);

    SessionRepository::save_session(store, &session_id, &conversation).unwrap();

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

        let edge = Edge::new(node1_id, node2_id, "connects".to_string(), 1.0);
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
    let edge_id = EdgeId::new(node1_id, node2_id, "connects".to_string());
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
        let edge = Edge::new(node1_id, node2_id, "connects".to_string(), 1.0);
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
