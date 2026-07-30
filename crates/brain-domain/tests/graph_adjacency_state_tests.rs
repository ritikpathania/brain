use brain_domain::bkf::*;
use brain_domain::identifiers::*;
use brain_domain::projection::graph_adjacency::*;
use uuid::Uuid;

#[test]
fn test_graph_adjacency_state_insert_lookup_and_prune() {
    let mut state = GraphAdjacencyState::default();
    let edge_id = GraphEdgeId(FactVersionId(Uuid::new_v4()));
    let source = GraphNodeId(EntityId::new());
    let target = GraphNodeId(EntityId::new());

    let now = Timestamp::now();
    let record = EdgeRecord {
        id: edge_id.clone(),
        source: source.clone(),
        target: target.clone(),
        predicate: PredicateId(Uuid::new_v4()),
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
    };

    state.insert_edge(record);
    assert_eq!(state.neighbors_out(&source), &[edge_id.clone()]);
    assert_eq!(state.neighbors_in(&target), &[edge_id.clone()]);
    assert_eq!(state.degree(&source).out_degree, 1);
    assert_eq!(state.degree(&target).in_degree, 1);

    // Verify degree consistency invariant
    assert_eq!(state.degree(&source).out_degree, state.neighbors_out(&source).len());
    assert_eq!(state.degree(&target).in_degree, state.neighbors_in(&target).len());

    state.remove_edge(&edge_id);
    assert!(state.neighbors_out(&source).is_empty());
    assert!(state.neighbors_in(&target).is_empty());
    assert_eq!(state.degree(&source).out_degree, 0);
}
