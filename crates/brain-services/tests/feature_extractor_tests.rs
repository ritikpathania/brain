use brain_core::repositories::NodeRepository;
use brain_core::retrieval::RetrievalRequest;
use brain_domain::{
    temporal::{RecencyPolicy, TemporalEdge, TimePoint},
    Edge, Node, NodeId, NodeType, RelationKind,
};
use brain_services::retrieval::feature_extractor::{DefaultFeatureExtractor, FeatureExtractor};
use brain_storage::TestStorage;

#[test]
fn test_default_feature_extractor_values_and_isolation() {
    let test_store = TestStorage::new();
    let sqlite = test_store.storage();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    NodeRepository::save(
        sqlite,
        &Node::new(node_a, "UniqueQuerySemantic".to_string(), NodeType::Concept),
    )
    .unwrap();
    NodeRepository::save(
        sqlite,
        &Node::new(node_b, "OtherEntity".to_string(), NodeType::Concept),
    )
    .unwrap();

    let request = RetrievalRequest {
        session_id: brain_domain::SessionId::new(),
        query: "UniqueQuerySemantic".to_string(),
        limit: 2,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let nodes = vec![
        NodeRepository::find_by_id(sqlite, &node_a)
            .unwrap()
            .unwrap(),
        NodeRepository::find_by_id(sqlite, &node_b)
            .unwrap()
            .unwrap(),
    ];

    let ref_time = TimePoint::from_unix_seconds(1620000000);
    let policy = RecencyPolicy::Linear {
        horizon_secs: 100.0,
    };
    let extractor = DefaultFeatureExtractor::new(ref_time, policy);

    // 1. Initial extraction with zero temporal edges
    let raw1 = extractor.extract(&request, &nodes, &[], sqlite).unwrap();
    assert_eq!(raw1.len(), 2);
    // Node A matches query, Node B doesn't
    assert!(raw1[0].semantic > 0.0);
    assert_eq!(raw1[1].semantic, 0.0);
    // Graph and temporal features are zero since no edges
    assert_eq!(raw1[0].graph, 0.0);
    assert_eq!(raw1[0].temporal, 0.0);

    // 2. Feature Isolation & Update: Add temporal edges for Node A (does not change semantic)
    let temp_edge = TemporalEdge {
        edge: Edge::new(node_a, node_b, RelationKind::AssociatedWith, 1.0),
        validity: brain_domain::temporal::TemporalValidity::new(vec![]),
        observed_at: TimePoint::from_unix_seconds(1620000000),
    };

    // Save active edge to satisfy repos.edges().get_connections view
    use brain_core::repositories::EdgeRepository;
    EdgeRepository::save(sqlite, &temp_edge.edge).unwrap();

    let raw2 = extractor
        .extract(&request, &nodes, &[temp_edge], sqlite)
        .unwrap();
    assert_eq!(raw2.len(), 2);
    // Graph connections are updated
    assert_eq!(raw2[0].graph, 1.0);
    assert_eq!(raw2[0].temporal, 1.0);
    // Feature Isolation Check: semantic score must remain exactly identical to raw1[0].semantic
    assert_eq!(raw2[0].semantic, raw1[0].semantic);

    test_store.assert_clean();
}
