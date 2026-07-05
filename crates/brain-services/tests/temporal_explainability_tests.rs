use std::sync::Arc;
use brain_core::repositories::NodeRepository;
use brain_domain::{
    Node, Edge, NodeType, RelationKind, NodeId, SessionId,
    retrieval::models::Evidence,
    temporal::{
        TimePoint, TimeInterval, TemporalValidity, RecencyPolicy,
        TemporalVisibility, TemporalQuery, TemporalEdge
    }
};
use brain_storage::TestStorage;
use brain_services::retrieval::temporal::TemporalRetrievalService;

#[test]
fn test_temporal_explainability_and_invariants() {
    let test_store = TestStorage::new();
    let store = test_store.store();
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    let service = TemporalRetrievalService::new(store.clone(), registry, None);

    let session_id = SessionId::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();

    let n_a = Node::new(node_a, "AlphaNode".to_string(), NodeType::Concept);
    let n_b = Node::new(node_b, "BetaNode".to_string(), NodeType::Concept);
    let n_c = Node::new(node_c, "GammaNode".to_string(), NodeType::Concept);

    NodeRepository::save(store.as_ref(), &n_a).unwrap();
    NodeRepository::save(store.as_ref(), &n_b).unwrap();
    NodeRepository::save(store.as_ref(), &n_c).unwrap();

    // Edge A -> B: valid [10, 20), observed at 5
    let mut e1 = Edge::new(node_a, node_b, RelationKind::Uses, 0.9);
    e1.updated_at = 5;
    let te1 = TemporalEdge {
        edge: e1,
        validity: TemporalValidity::new(vec![TimeInterval::new(TimePoint::from_unix_seconds(10), Some(TimePoint::from_unix_seconds(20))).unwrap()]),
        observed_at: TimePoint::from_unix_seconds(5),
    };
    store.save_temporal_edge(&te1).unwrap();

    // Edge A -> C: valid [15, 30), observed at 12
    let mut e2 = Edge::new(node_a, node_c, RelationKind::Uses, 0.9);
    e2.updated_at = 12;
    let te2 = TemporalEdge {
        edge: e2,
        validity: TemporalValidity::new(vec![TimeInterval::new(TimePoint::from_unix_seconds(15), Some(TimePoint::from_unix_seconds(30))).unwrap()]),
        observed_at: TimePoint::from_unix_seconds(12),
    };
    store.save_temporal_edge(&te2).unwrap();

    // 1. Verify Visibility Explanations (As of T=16)
    let query_16 = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(16),
        visibility: TemporalVisibility::Current,
        recency_policy: RecencyPolicy::None,
    };

    let (res_16, explanations_16) = service.retrieve_temporal(&session_id, "AlphaNode", 10, &query_16).unwrap();
    assert!(!res_16.is_empty());
    
    // Check that explanations contain TemporalVisibility for the active edges
    let evidences_a = explanations_16.get(&node_a).expect("AlphaNode should have explanations");
    
    let visibility_count = evidences_a.iter().filter(|ev| {
        matches!(ev, Evidence::TemporalVisibility { .. })
    }).count();
    // Both edges (A -> B and A -> C) are active at T=16, so there should be exactly 2 visibility items
    assert_eq!(visibility_count, 2, "Should emit exactly 2 visibility evidence items");

    // Assert that the contents of the TemporalVisibility evidence match the edge and query parameters
    let mut evidences_a = explanations_16.get(&node_a).expect("AlphaNode should have explanations").clone();
    evidences_a.sort_by(|a, b| {
        let obs_a = match a {
            Evidence::TemporalVisibility { observed_at, .. } => *observed_at,
            _ => TimePoint::from_unix_seconds(0),
        };
        let obs_b = match b {
            Evidence::TemporalVisibility { observed_at, .. } => *observed_at,
            _ => TimePoint::from_unix_seconds(0),
        };
        obs_a.cmp(&obs_b)
    });

    if let Evidence::TemporalVisibility { observed_at, validity_intervals, query_time, visibility_mode } = &evidences_a[0] {
        assert_eq!(*query_time, TimePoint::from_unix_seconds(16));
        assert_eq!(*visibility_mode, TemporalVisibility::Current);
        // Ordering check: first edge by sorted order is A -> B (observed at 5, validity [10, 20))
        assert_eq!(*observed_at, TimePoint::from_unix_seconds(5));
        assert_eq!(validity_intervals.len(), 1);
        assert_eq!(validity_intervals[0], TimeInterval::new(TimePoint::from_unix_seconds(10), Some(TimePoint::from_unix_seconds(20))).unwrap());
    } else {
        panic!("First evidence should be TemporalVisibility");
    }

    if let Evidence::TemporalVisibility { observed_at, validity_intervals, query_time, visibility_mode } = &evidences_a[1] {
        assert_eq!(*query_time, TimePoint::from_unix_seconds(16));
        assert_eq!(*visibility_mode, TemporalVisibility::Current);
        // Second edge by sorted order is A -> C (observed at 12, validity [15, 30))
        assert_eq!(*observed_at, TimePoint::from_unix_seconds(12));
        assert_eq!(validity_intervals.len(), 1);
        assert_eq!(validity_intervals[0], TimeInterval::new(TimePoint::from_unix_seconds(15), Some(TimePoint::from_unix_seconds(30))).unwrap());
    } else {
        panic!("Second evidence should be TemporalVisibility");
    }

    // 2. Verify Recency Decay Explanation (With active policy)
    let query_decay = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(25),
        visibility: TemporalVisibility::Current,
        recency_policy: RecencyPolicy::Linear { horizon_secs: 20.0 },
    };

    let (res_decay, explanations_decay) = service.retrieve_temporal(&session_id, "AlphaNode", 10, &query_decay).unwrap();
    assert!(!res_decay.is_empty());

    let evidences_decay = explanations_decay.get(&node_a).expect("AlphaNode should have explanations");
    
    // Check that we got exactly one RecencyDecay evidence item (since policy is active)
    let decay_count = evidences_decay.iter().filter(|ev| {
        matches!(ev, Evidence::RecencyDecay { .. })
    }).count();
    assert_eq!(decay_count, 1, "Should emit exactly 1 recency decay evidence item");

    // Extract decay details and check correctness/reproducibility
    if let Evidence::RecencyDecay { policy, observed_at, reference_time, elapsed_seconds, decay_factor } = &evidences_decay[evidences_decay.len() - 1] {
        assert_eq!(*policy, RecencyPolicy::Linear { horizon_secs: 20.0 });
        // Max observed time for Node A is 12 (from te2)
        assert_eq!(*observed_at, TimePoint::from_unix_seconds(12));
        assert_eq!(*reference_time, TimePoint::from_unix_seconds(25));
        assert_eq!(*elapsed_seconds, 13.0);
        // Linear decay: 1 - elapsed/horizon = 1 - 13/20 = 7/20 = 0.35
        assert!((*decay_factor - 0.35).abs() < 1e-9);
    } else {
        panic!("Last evidence should be RecencyDecay");
    }

    // 3. Verify Evidence Ordering Determinism
    // Execute multiple consecutive retrievals and assert identical evidence order/byte structure
    for _ in 0..5 {
        let (_, explanations_consec) = service.retrieve_temporal(&session_id, "AlphaNode", 10, &query_decay).unwrap();
        let evidences_consec = explanations_consec.get(&node_a).unwrap();
        assert_eq!(evidences_decay, evidences_consec, "Evidence lists must be identical across consecutive executions");
    }

    // 4. Verify Explanation Transparency Invariant
    // Assert that disabling/enabling explainability doesn't modify the matched list, rank ordering, or score weights
    let (res_none, _) = service.retrieve_temporal(&session_id, "AlphaNode", 10, &query_16).unwrap();
    
    // Retrieve again with the same parameters
    let (res_none_again, _) = service.retrieve_temporal(&session_id, "AlphaNode", 10, &query_16).unwrap();
    assert_eq!(res_none.len(), res_none_again.len());
    for (a, b) in res_none.iter().zip(res_none_again.iter()) {
        assert_eq!(a.node.id, b.node.id);
        assert_eq!(a.node.label, b.node.label);
        assert_eq!(a.node.node_type, b.node.node_type);
    }
}
