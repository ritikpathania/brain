use brain_domain::{
    temporal::{
        RecencyPolicy, TemporalEdge, TemporalProjector, TemporalQuery, TemporalValidity,
        TemporalVisibility, TimeInterval, TimePoint,
    },
    Edge, KnowledgeGraph, Node, NodeId, NodeType, RelationKind,
};

fn node(id: NodeId, label: &str, timestamp: u64) -> Node {
    let mut n = Node::new(id, label.to_string(), NodeType::Concept);
    n.updated_at = timestamp;
    n
}

fn edge(source: NodeId, target: NodeId, relation: RelationKind, weight: f64) -> Edge {
    Edge::new(source, target, relation, weight)
}

fn build_temporal_graph() -> (
    KnowledgeGraph,
    Vec<TemporalEdge>,
    NodeId, // n1 (created at t=10)
    NodeId, // n2 (created at t=20)
    NodeId, // n3 (created at t=30)
) {
    let mut graph = KnowledgeGraph::new();

    let n1 = NodeId::new();
    let n2 = NodeId::new();
    let n3 = NodeId::new();

    graph.add_node(node(n1, "Node 1", 10));
    graph.add_node(node(n2, "Node 2", 20));
    graph.add_node(node(n3, "Node 3", 30));

    // Create edges
    let e1 = edge(n1, n2, RelationKind::Uses, 1.0);
    let e2 = edge(n2, n3, RelationKind::Uses, 0.8);

    // Add them to base graph
    let _ = graph.add_edge(e1.clone());
    let _ = graph.add_edge(e2.clone());

    // Wrap in TemporalEdge
    // te1 observed at t=25, valid [20, 40)
    let te1 = TemporalEdge {
        edge: e1,
        validity: TemporalValidity::new(vec![TimeInterval::new(
            TimePoint::from_unix_seconds(20),
            Some(TimePoint::from_unix_seconds(40)),
        )
        .unwrap()]),
        observed_at: TimePoint::from_unix_seconds(25),
    };

    // te2 observed at t=35, valid [30, 50)
    let te2 = TemporalEdge {
        edge: e2,
        validity: TemporalValidity::new(vec![TimeInterval::new(
            TimePoint::from_unix_seconds(30),
            Some(TimePoint::from_unix_seconds(50)),
        )
        .unwrap()]),
        observed_at: TimePoint::from_unix_seconds(35),
    };

    let temporal_edges = vec![te1, te2];

    (graph, temporal_edges, n1, n2, n3)
}

#[test]
fn test_temporal_graph_projection_t15() {
    let (graph, temp_edges, n1, n2, n3) = build_temporal_graph();

    // Query at T = 15
    let query = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(15),
        visibility: TemporalVisibility::Historical,
        recency_policy: RecencyPolicy::None,
    };

    let projected = TemporalProjector::project_graph(&graph, &temp_edges, &query);

    // At T = 15:
    // Only Node 1 (updated_at=10) should exist.
    // Node 2 (updated_at=20) and Node 3 (updated_at=30) should not.
    // No edges should exist since endpoints are missing or edges not yet observed.
    assert!(projected.nodes.contains_key(&n1));
    assert!(!projected.nodes.contains_key(&n2));
    assert!(!projected.nodes.contains_key(&n3));
    assert!(projected.edges.is_empty());
}

#[test]
fn test_temporal_graph_projection_t27_historical() {
    let (graph, temp_edges, n1, n2, n3) = build_temporal_graph();

    // Query at T = 27
    let query = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(27),
        visibility: TemporalVisibility::Historical,
        recency_policy: RecencyPolicy::None,
    };

    let projected = TemporalProjector::project_graph(&graph, &temp_edges, &query);

    // At T = 27:
    // Node 1 (10) and Node 2 (20) exist. Node 3 (30) does not.
    // Edge 1 (observed at 25, start=20 <= 27) should exist.
    // Edge 2 (observed at 35) does not.
    assert!(projected.nodes.contains_key(&n1));
    assert!(projected.nodes.contains_key(&n2));
    assert!(!projected.nodes.contains_key(&n3));

    assert_eq!(projected.edges.len(), 1);
}

#[test]
fn test_temporal_graph_projection_t45_current_vs_historical() {
    let (graph, temp_edges, _n1, n2, n3) = build_temporal_graph();

    // Query at T = 45 with Current visibility (Edge 1 expired at 40)
    let query_current = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(45),
        visibility: TemporalVisibility::Current,
        recency_policy: RecencyPolicy::None,
    };

    let projected_current = TemporalProjector::project_graph(&graph, &temp_edges, &query_current);

    // At T = 45 with Current:
    // Edge 1 expired at 40 -> not visible.
    // Edge 2 valid until 50 -> visible.
    assert_eq!(projected_current.edges.len(), 1);
    let edge_keys: Vec<_> = projected_current.edges.keys().collect();
    assert_eq!(edge_keys[0].source, n2);
    assert_eq!(edge_keys[0].target, n3);

    // Query at T = 45 with Historical visibility (both edges ever active before/at 45)
    let query_historical = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(45),
        visibility: TemporalVisibility::Historical,
        recency_policy: RecencyPolicy::None,
    };

    let projected_historical =
        TemporalProjector::project_graph(&graph, &temp_edges, &query_historical);

    // Under historical visibility, both edges should exist.
    assert_eq!(projected_historical.edges.len(), 2);
}
