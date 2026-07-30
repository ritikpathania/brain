use brain_domain::projection::graph_adjacency::*;
use brain_domain::projection::*;

#[test]
fn test_graph_adjacency_services_reexport() {
    let reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    assert_eq!(reducer.id().as_str(), "adj");
}
