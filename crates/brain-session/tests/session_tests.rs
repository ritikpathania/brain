use brain_domain::{Node, NodeId, NodeType, SessionId};
use brain_session::{EpochId, SessionCacheManager, SessionContext, StmNode};
use std::collections::HashMap;

#[test]
fn test_session_context_sliding_window() {
    let session_id = SessionId::new();
    let mut session = SessionContext::new(session_id);

    assert_eq!(session.session_id(), session_id);
    assert_eq!(session.current_epoch(), EpochId(0));
    assert!(session.is_empty());
    assert_eq!(session.len(), 0);

    let node1 = Node::new(NodeId::new(), "API Key".to_string(), NodeType::Concept);
    let stm_node1 = session.ingest(node1);

    assert_eq!(stm_node1.epoch, EpochId(0));
    assert_eq!(session.len(), 1);
    assert!(!session.is_empty());

    let nodes: Vec<&StmNode> = session.iter().collect();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].node.label, "API Key");
}

#[test]
fn test_session_context_search_exact() {
    let mut session = SessionContext::new(SessionId::new());

    // Ingest Node A
    let node_a = Node::new(
        NodeId::new(),
        "API Key environment config".to_string(),
        NodeType::Concept,
    );
    let stm_node_a = session.ingest(node_a);

    // Ingest Node B
    let node_b = Node::new(
        NodeId::new(),
        "SQLite database settings".to_string(),
        NodeType::Concept,
    );
    let stm_node_b = session.ingest(node_b);

    // Ingest Node C with properties
    let mut props = HashMap::new();
    props.insert(
        "desc".to_string(),
        serde_json::Value::String("environment API key storage".to_string()),
    );
    let node_c =
        Node::new(NodeId::new(), "Storage".to_string(), NodeType::Concept).with_properties(props);
    let stm_node_c = session.ingest(node_c);

    // Exact matches
    let results = session.query("api key");
    assert_eq!(results.len(), 2);
    // Deterministic insertion order: Node A comes before Node C
    assert_eq!(results[0], stm_node_a);
    assert_eq!(results[1], stm_node_c);

    // Match database
    let results2 = session.query("database");
    assert_eq!(results2.len(), 1);
    assert_eq!(results2[0], stm_node_b);
}

#[test]
fn test_session_context_epoch_rotation() {
    let mut session = SessionContext::new(SessionId::new());

    let old_epoch = session.rotate_epoch();
    assert_eq!(old_epoch, EpochId(0));
    assert_eq!(session.current_epoch(), EpochId(1));

    let old_epoch2 = session.rotate_epoch();
    assert_eq!(old_epoch2, EpochId(1));
    assert_eq!(session.current_epoch(), EpochId(2));
}

#[test]
fn test_session_context_eviction_invariant() {
    let mut session = SessionContext::new(SessionId::new());

    // Ingest Node A into Epoch 0
    let node_a = Node::new(
        NodeId::new(),
        "Node A content".to_string(),
        NodeType::Concept,
    );
    let stm_node_a = session.ingest(node_a);

    session.rotate_epoch(); // Moves to Epoch 1

    // Ingest Node B into Epoch 1
    let node_b = Node::new(
        NodeId::new(),
        "Node B content".to_string(),
        NodeType::Concept,
    );
    let stm_node_b = session.ingest(node_b);

    // Before drain, both are searchable
    assert_eq!(session.query("content").len(), 2);

    // Drain Epoch 0
    let drained = session.drain_epoch(EpochId(0));
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0], stm_node_a);

    // Node A must never be returned after eviction
    let results = session.query("content");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], stm_node_b);
    assert_eq!(session.len(), 1);
}

#[test]
fn test_session_context_rebuild_invariant() {
    let mut session = SessionContext::new(SessionId::new());

    // Ingest Node A & B into Epoch 0
    let node_a = Node::new(NodeId::new(), "Target Alpha".to_string(), NodeType::Concept);
    let _stm_node_a = session.ingest(node_a);

    let node_b = Node::new(NodeId::new(), "Target Beta".to_string(), NodeType::Concept);
    let _stm_node_b = session.ingest(node_b);

    session.rotate_epoch(); // Rotate to Epoch 1

    // Ingest Node C into Epoch 1
    let node_c = Node::new(NodeId::new(), "Target Gamma".to_string(), NodeType::Concept);
    let stm_node_c = session.ingest(node_c);

    // Verify A, B, and C are all searchable before draining
    let before_drain = session.query("Target");
    assert_eq!(before_drain.len(), 3);

    // Drain Epoch 0
    let drained = session.drain_epoch(EpochId(0));
    assert_eq!(drained.len(), 2);

    // Verify A and B disappear, but C remains searchable
    let after_drain = session.query("Target");
    assert_eq!(after_drain.len(), 1);
    assert_eq!(after_drain[0], stm_node_c);

    assert_eq!(session.query("Alpha").len(), 0);
    assert_eq!(session.query("Beta").len(), 0);
    assert_eq!(session.query("Gamma").len(), 1);
}

#[test]
fn test_session_context_query_mutability_invariant() {
    let mut session = SessionContext::new(SessionId::new());

    let node_a = Node::new(
        NodeId::new(),
        "Query testing text".to_string(),
        NodeType::Concept,
    );
    session.ingest(node_a);
    let node_b = Node::new(
        NodeId::new(),
        "Query testing verify".to_string(),
        NodeType::Concept,
    );
    session.ingest(node_b);

    // query() is a pure read operation. Repeated queries must return identical results.
    let results_1 = session.query("testing");
    let results_2 = session.query("testing");

    assert_eq!(results_1, results_2);
    assert_eq!(results_1.len(), 2);
}

#[test]
fn test_session_cache_manager() {
    let manager = SessionCacheManager::new();
    let session_id = SessionId::new();

    // Context gets created on lookup
    let context_arc1 = manager.get_or_create(session_id);
    {
        let mut ctx = context_arc1.write().unwrap();
        ctx.ingest(Node::new(
            NodeId::new(),
            "Some text".to_string(),
            NodeType::Concept,
        ));
    }

    // Context is re-used on second lookup
    let context_arc2 = manager.get_or_create(session_id);
    assert_eq!(context_arc2.read().unwrap().len(), 1);

    // Context gets cleaned up on remove
    manager.remove(&session_id);
    let context_arc3 = manager.get_or_create(session_id);
    assert_eq!(context_arc3.read().unwrap().len(), 0);
}
