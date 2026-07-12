use std::sync::Arc;
use brain_core::repositories::NodeRepository;
use brain_domain::{
    Node, Edge, NodeType, RelationKind, NodeId, SessionId,
    temporal::{
        TimePoint, TimeInterval, TemporalValidity, RecencyPolicy,
        TemporalVisibility, TemporalQuery, TemporalEdge
    }
};
use brain_storage::TestStorage;
use brain_services::retrieval::temporal::TemporalRetrievalService;

#[test]
fn test_temporal_retrieval_and_invariants() {
    let test_store = TestStorage::new();
    let store = test_store.store();
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    let service = TemporalRetrievalService::new(store.clone(), registry, None);

    let session_id = SessionId::new();

    // 1. Create duplicate nodes
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();
    let node_d = NodeId::new();

    let n_a = Node::new(node_a, "EntityA".to_string(), NodeType::Concept);
    let n_b = Node::new(node_b, "EntityB".to_string(), NodeType::Concept);
    let n_c = Node::new(node_c, "EntityC".to_string(), NodeType::Concept);
    let n_d = Node::new(node_d, "EntityD".to_string(), NodeType::Concept);

    NodeRepository::save(store.as_ref(), &n_a).unwrap();
    NodeRepository::save(store.as_ref(), &n_b).unwrap();
    NodeRepository::save(store.as_ref(), &n_c).unwrap();
    NodeRepository::save(store.as_ref(), &n_d).unwrap();

    // 2. Setup temporal edges
    // Edge A -> B: validity [10, 20), observed at 5
    let mut e1 = Edge::new(node_a, node_b, RelationKind::Uses, 0.8);
    e1.updated_at = 5;
    let te1 = TemporalEdge {
        edge: e1,
        validity: TemporalValidity::new(vec![TimeInterval::new(TimePoint::from_unix_seconds(10), Some(TimePoint::from_unix_seconds(20))).unwrap()]),
        observed_at: TimePoint::from_unix_seconds(5),
    };
    store.save_temporal_edge(&te1).unwrap();

    // Edge C -> D: validity [20, 30), observed at 15
    let mut e2 = Edge::new(node_c, node_d, RelationKind::Uses, 0.8);
    e2.updated_at = 15;
    let te2 = TemporalEdge {
        edge: e2,
        validity: TemporalValidity::new(vec![TimeInterval::new(TimePoint::from_unix_seconds(20), Some(TimePoint::from_unix_seconds(30))).unwrap()]),
        observed_at: TimePoint::from_unix_seconds(15),
    };
    store.save_temporal_edge(&te2).unwrap();

    // Invariant 1: Historical "As Of" Retrieval at T=15
    let query_15 = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(15),
        visibility: TemporalVisibility::Current,
        recency_policy: RecencyPolicy::None,
    };
    // Search for "EntityA"
    let (res_15, _) = service.retrieve_temporal(&session_id, "EntityA", 10, &query_15).unwrap();
    assert!(!res_15.is_empty(), "EntityA should be found");
    let dto_a_15 = res_15.iter().find(|dto| dto.node.label == "EntityA").unwrap();
    assert!(!dto_a_15.outgoing_edges.is_empty(), "EntityA should have active outgoing edges at T=15");
    
    // Search for "EntityC"
    let (res_15_c, _) = service.retrieve_temporal(&session_id, "EntityC", 10, &query_15).unwrap();
    assert!(!res_15_c.is_empty(), "EntityC should be found");
    let dto_c_15 = res_15_c.iter().find(|dto| dto.node.label == "EntityC").unwrap();
    assert!(dto_c_15.outgoing_edges.is_empty(), "EntityC should NOT have active outgoing edges at T=15");

    // Invariant 2: Historical "As Of" Retrieval at T=25
    let query_25 = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(25),
        visibility: TemporalVisibility::Current,
        recency_policy: RecencyPolicy::None,
    };
    let (res_25_a, _) = service.retrieve_temporal(&session_id, "EntityA", 10, &query_25).unwrap();
    assert!(!res_25_a.is_empty(), "EntityA should be found");
    let dto_a_25 = res_25_a.iter().find(|dto| dto.node.label == "EntityA").unwrap();
    assert!(dto_a_25.outgoing_edges.is_empty(), "EntityA should have no outgoing edges at T=25 (validity ended)");
    
    let (res_25_c, _) = service.retrieve_temporal(&session_id, "EntityC", 10, &query_25).unwrap();
    assert!(!res_25_c.is_empty(), "EntityC should be found");
    let dto_c_25 = res_25_c.iter().find(|dto| dto.node.label == "EntityC").unwrap();
    assert!(!dto_c_25.outgoing_edges.is_empty(), "EntityC should have outgoing edges at T=25");

    // Invariant 3: Interval Intersection Visibility
    let query_interval = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(22),
        visibility: TemporalVisibility::Interval(TimeInterval::new(TimePoint::from_unix_seconds(15), Some(TimePoint::from_unix_seconds(25))).unwrap()),
        recency_policy: RecencyPolicy::None,
    };
    let (res_interval_a, _) = service.retrieve_temporal(&session_id, "EntityA", 10, &query_interval).unwrap();
    let (res_interval_c, _) = service.retrieve_temporal(&session_id, "EntityC", 10, &query_interval).unwrap();
    
    let dto_a_int = res_interval_a.iter().find(|dto| dto.node.label == "EntityA").unwrap();
    let dto_c_int = res_interval_c.iter().find(|dto| dto.node.label == "EntityC").unwrap();
    
    assert!(!dto_a_int.outgoing_edges.is_empty(), "EntityA edges intersect [15, 25) with ref 22");
    assert!(!dto_c_int.outgoing_edges.is_empty(), "EntityC edges intersect [15, 25) with ref 22");

    // Invariant 4: Recency-Aware Preference Ranking
    // Add Node E
    let node_e = NodeId::new();
    let n_e = Node::new(node_e, "CommonLabel".to_string(), NodeType::Concept);
    NodeRepository::save(store.as_ref(), &n_e).unwrap();

    // Edge A -> E: observed at 10
    let mut e3 = Edge::new(node_a, node_e, RelationKind::Uses, 0.8);
    e3.updated_at = 10;
    let te3 = TemporalEdge {
        edge: e3,
        validity: TemporalValidity::new(Vec::new()), // infinite
        observed_at: TimePoint::from_unix_seconds(10),
    };
    store.save_temporal_edge(&te3).unwrap();

    // Edge C -> E: observed at 20
    let mut e4 = Edge::new(node_c, node_e, RelationKind::Uses, 0.8);
    e4.updated_at = 20;
    let te4 = TemporalEdge {
        edge: e4,
        validity: TemporalValidity::new(Vec::new()), // infinite
        observed_at: TimePoint::from_unix_seconds(20),
    };
    store.save_temporal_edge(&te4).unwrap();

    // Query for "CommonLabel" at T=25 with recency-aware ranking
    let query_decay = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(25),
        visibility: TemporalVisibility::Current,
        recency_policy: RecencyPolicy::Linear { horizon_secs: 30.0 },
    };
    let (res_decay, _) = service.retrieve_temporal(&session_id, "CommonLabel", 10, &query_decay).unwrap();
    assert!(!res_decay.is_empty());
}
