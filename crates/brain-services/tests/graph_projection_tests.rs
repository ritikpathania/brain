use brain_core::events::CorrelationId;
use brain_core::projection::{ProjectionContext, Projector};
use brain_domain::{Edge, EpochId, KnowledgeGraph, Node, NodeId, NodeType, RelationKind};
use brain_services::graph::projections::{
    ClusterProjector, ClusterQuery, NeighborhoodProjector, NeighborhoodQuery, PathProjector,
    PathQuery,
};
use std::collections::HashSet;

fn node(id: NodeId, label: &str) -> Node {
    Node::new(id, label.to_string(), NodeType::Concept)
}

fn build_test_graph() -> (
    KnowledgeGraph,
    NodeId, // n1
    NodeId, // n2
    NodeId, // n3
    NodeId, // n4 (disconnected component)
) {
    let mut graph = KnowledgeGraph::new();

    let n1 = NodeId::new();
    let n2 = NodeId::new();
    let n3 = NodeId::new();
    let n4 = NodeId::new();

    graph.add_node(node(n1, "Node 1"));
    graph.add_node(node(n2, "Node 2"));
    graph.add_node(node(n3, "Node 3"));
    graph.add_node(node(n4, "Node 4"));

    // n1 <-> n2
    graph
        .add_edge(Edge::new(n1, n2, RelationKind::Uses, 1.0))
        .unwrap();
    // n2 <-> n3
    graph
        .add_edge(Edge::new(n2, n3, RelationKind::Uses, 0.8))
        .unwrap();

    (graph, n1, n2, n3, n4)
}

fn context<'a, Q: brain_core::projection::ProjectionQuery>(
    graph: &'a KnowledgeGraph,
    query: &'a Q,
) -> ProjectionContext<'a, Q> {
    ProjectionContext {
        graph,
        epoch: EpochId(1),
        query,
        correlation_id: CorrelationId::new_v4(),
    }
}

// ─── Neighborhood Projection Tests ───────────────────────────────────────────

#[test]
fn test_neighborhood_projection_depth_one() {
    let (graph, n1, n2, _n3, _n4) = build_test_graph();
    let query = NeighborhoodQuery {
        center_node_id: n1,
        depth: 1,
    };
    let projector = NeighborhoodProjector;
    let result = projector.project(&context(&graph, &query));

    assert_eq!(result.nodes.len(), 2);
    let ids: HashSet<NodeId> = result.nodes.iter().map(|n| n.id).collect();
    assert!(ids.contains(&n1));
    assert!(ids.contains(&n2));

    assert_eq!(result.edges.len(), 1);
    assert_eq!(result.edges[0].source, n1);
    assert_eq!(result.edges[0].target, n2);
}

#[test]
fn test_neighborhood_projection_depth_two() {
    let (graph, n1, n2, n3, _n4) = build_test_graph();
    let query = NeighborhoodQuery {
        center_node_id: n1,
        depth: 2,
    };
    let projector = NeighborhoodProjector;
    let result = projector.project(&context(&graph, &query));

    assert_eq!(result.nodes.len(), 3);
    let ids: HashSet<NodeId> = result.nodes.iter().map(|n| n.id).collect();
    assert!(ids.contains(&n1));
    assert!(ids.contains(&n2));
    assert!(ids.contains(&n3));

    assert_eq!(result.edges.len(), 2);
}

#[test]
fn test_neighborhood_projection_non_existent() {
    let (graph, _n1, _n2, _n3, _n4) = build_test_graph();
    let query = NeighborhoodQuery {
        center_node_id: NodeId::new(),
        depth: 2,
    };
    let projector = NeighborhoodProjector;
    let result = projector.project(&context(&graph, &query));

    assert!(result.nodes.is_empty());
    assert!(result.edges.is_empty());
}

// ─── Path Projection Tests ───────────────────────────────────────────────────

#[test]
fn test_path_projection_success() {
    let (graph, n1, n2, n3, _n4) = build_test_graph();
    let query = PathQuery {
        source_node_id: n1,
        target_node_id: n3,
    };
    let projector = PathProjector;
    let result = projector.project(&context(&graph, &query));

    let path = result.path.expect("path should exist");
    assert_eq!(path.len(), 3);
    assert_eq!(path[0].id, n1);
    assert_eq!(path[1].id, n2);
    assert_eq!(path[2].id, n3);

    assert_eq!(result.edges.len(), 2);
}

#[test]
fn test_path_projection_same_node() {
    let (graph, n1, _n2, _n3, _n4) = build_test_graph();
    let query = PathQuery {
        source_node_id: n1,
        target_node_id: n1,
    };
    let projector = PathProjector;
    let result = projector.project(&context(&graph, &query));

    let path = result.path.expect("path should exist");
    assert_eq!(path.len(), 1);
    assert_eq!(path[0].id, n1);
    assert!(result.edges.is_empty());
}

#[test]
fn test_path_projection_unreachable() {
    let (graph, n1, _n2, _n3, n4) = build_test_graph();
    let query = PathQuery {
        source_node_id: n1,
        target_node_id: n4,
    };
    let projector = PathProjector;
    let result = projector.project(&context(&graph, &query));

    assert!(result.path.is_none());
    assert!(result.edges.is_empty());
}

// ─── Cluster Projection Tests ────────────────────────────────────────────────

#[test]
fn test_cluster_projection_all() {
    let (graph, n1, n2, n3, n4) = build_test_graph();
    let query = ClusterQuery {
        min_cluster_size: None,
    };
    let projector = ClusterProjector;
    let result = projector.project(&context(&graph, &query));

    // We have 2 disjoint connected components:
    // Cluster A: {n1, n2, n3} (size 3)
    // Cluster B: {n4} (size 1)
    assert_eq!(result.clusters.len(), 2);

    let mut sizes: Vec<usize> = result.clusters.values().map(|c| c.len()).collect();
    sizes.sort();
    assert_eq!(sizes, vec![1, 3]);

    // Check cluster members
    let c_3 = result.clusters.values().find(|c| c.len() == 3).unwrap();
    assert!(c_3.contains(&n1));
    assert!(c_3.contains(&n2));
    assert!(c_3.contains(&n3));

    let c_1 = result.clusters.values().find(|c| c.len() == 1).unwrap();
    assert!(c_1.contains(&n4));
}

#[test]
fn test_cluster_projection_filtered() {
    let (graph, _n1, _n2, _n3, _n4) = build_test_graph();
    let query = ClusterQuery {
        min_cluster_size: Some(2),
    };
    let projector = ClusterProjector;
    let result = projector.project(&context(&graph, &query));

    // Only the component of size 3 should remain, size 1 component filtered out.
    assert_eq!(result.clusters.len(), 1);
    assert_eq!(result.clusters.values().next().unwrap().len(), 3);
}
