use brain_domain::{
    AStarConfig, AffectedElement, AnalyticsAlgorithm, Centrality, CentralityConfig,
    ClosenessConfig, ClosenessVariant, Complexity, ConnectedComponents, ConnectedComponentsConfig,
    ConnectivityConfig, CycleDetectionConfig, Distribution, DistributionConfig, Edge, EdgeId,
    GraphAnalyticsContext, GraphQueryEngine, GraphValidator, HeuristicProvider, KnowledgeGraph,
    Node, NodeId, NodeType, PageRankConfig, PathQuery, ProvenanceConfig, ProvenanceSource,
    ProvenanceStatistics, ProvenanceStats, RelationKind, RelationRegistry, SccConfig,
    ShortestPathConfig, UniformWeightProvider, ZeroHeuristic,
};
use std::collections::HashSet;

fn create_test_registry() -> RelationRegistry {
    RelationRegistry::default_embedded()
}

#[test]
fn test_derived_edges_query() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));

    // 1 extracted, 1 inferred
    let edge1 = Edge::new(node_a, node_b, RelationKind::Uses, 0.9);

    let mut edge2 = Edge::new(node_b, node_a, RelationKind::StoredIn, 0.8);
    edge2.provenance.source = ProvenanceSource::Inferred;

    graph.add_edge(edge1).unwrap();
    graph.add_edge(edge2).unwrap();

    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);

    let derived_edges: Vec<&Edge> = engine.find_derived_edges().collect();
    assert_eq!(derived_edges.len(), 1);
    assert_eq!(derived_edges[0].relation, RelationKind::StoredIn);
}

#[test]
fn test_diagnostics_for_element_filter() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    // Add an edge without adding the nodes first (triggers VAL-001 and VAL-002)
    let edge = Edge::new(node_a, node_b, RelationKind::Uses, 0.9);
    let edge_id = EdgeId::new(node_a, node_b, RelationKind::Uses.id());
    graph.edges.insert(edge_id.clone(), edge);

    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);

    let node_a_diagnostics = engine.find_diagnostics_for_element(&AffectedElement::Node(node_a));
    assert_eq!(node_a_diagnostics.len(), 1);
    assert_eq!(node_a_diagnostics[0].code, "VAL-001");

    let edge_diagnostics = engine.find_diagnostics_for_element(&AffectedElement::Edge(edge_id));
    assert_eq!(edge_diagnostics.len(), 2); // affects both VAL-001 and VAL-002
}

#[test]
fn test_find_paths_canonical_sorting_and_limits() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();
    let node_d = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_d, "NodeD".to_string(), NodeType::Concept));

    // Construct paths:
    // Path 1 (len 1): A -develops-> D
    // Path 2 (len 2): A -uses-> B, B -uses-> D
    // Path 3 (len 3): A -uses-> B, B -uses-> C, C -uses-> D
    let ab = Edge::new(node_a, node_b, RelationKind::Uses, 0.9);
    let bd = Edge::new(node_b, node_d, RelationKind::Uses, 0.9);
    let bc = Edge::new(node_b, node_c, RelationKind::Uses, 0.9);
    let cd = Edge::new(node_c, node_d, RelationKind::Uses, 0.9);
    let ad = Edge::new(node_a, node_d, RelationKind::Develops, 0.9);

    let id_ab = EdgeId::new(node_a, node_b, RelationKind::Uses.id());
    let id_bd = EdgeId::new(node_b, node_d, RelationKind::Uses.id());
    let id_bc = EdgeId::new(node_b, node_c, RelationKind::Uses.id());
    let id_cd = EdgeId::new(node_c, node_d, RelationKind::Uses.id());
    let id_ad = EdgeId::new(node_a, node_d, RelationKind::Develops.id());

    graph.add_edge(ab).unwrap();
    graph.add_edge(bd).unwrap();
    graph.add_edge(bc).unwrap();
    graph.add_edge(cd).unwrap();
    graph.add_edge(ad).unwrap();

    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);

    // 1. Query with max depth of 1 (should return only Path 1: A -> D)
    let query_depth1 = PathQuery::new().with_max_depth(1);
    let paths_depth1 = engine.find_paths(&node_a, &node_d, &query_depth1);
    assert_eq!(paths_depth1.len(), 1);
    assert_eq!(paths_depth1[0], vec![id_ad.clone()]);

    // 2. Query with max depth of 3 (should return all 3 paths)
    let query_depth3 = PathQuery::new().with_max_depth(3);
    let paths_depth3 = engine.find_paths(&node_a, &node_d, &query_depth3);
    assert_eq!(paths_depth3.len(), 3);

    // Verify Canonical Path Ordering Contract:
    // First by length (Path 1 [len 1], then Path 2 [len 2], then Path 3 [len 3])
    assert_eq!(paths_depth3[0].len(), 1);
    assert_eq!(paths_depth3[1].len(), 2);
    assert_eq!(paths_depth3[2].len(), 3);

    assert_eq!(paths_depth3[0], vec![id_ad.clone()]);
    assert_eq!(paths_depth3[1], vec![id_ab.clone(), id_bd.clone()]);
    assert_eq!(
        paths_depth3[2],
        vec![id_ab.clone(), id_bc.clone(), id_cd.clone()]
    );

    // 3. Query with relation filter (only allows Uses, filters out Path 1)
    let mut relations = HashSet::new();
    relations.insert(RelationKind::Uses.id());
    let query_relation = PathQuery::new().with_max_depth(3).with_relations(relations);
    let paths_relation = engine.find_paths(&node_a, &node_d, &query_relation);

    // Path 1 (develops) filtered out, leaving Path 2 and Path 3
    assert_eq!(paths_relation.len(), 2);
    assert_eq!(paths_relation[0], vec![id_ab.clone(), id_bd.clone()]);
    assert_eq!(
        paths_relation[1],
        vec![id_ab.clone(), id_bc.clone(), id_cd.clone()]
    );
}

#[test]
fn test_query_engine_analytics_algorithms() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();
    let node_d = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_d, "NodeD".to_string(), NodeType::Concept));

    // A -uses-> B, C -develops-> D
    // A and B are in component 1. C and D are in component 2.
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 0.9))
        .unwrap();
    graph
        .add_edge(Edge::new(node_c, node_d, RelationKind::Develops, 0.8))
        .unwrap();

    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);

    // 1. Connected Components (should return 2 components of size 2)
    let mut comps = engine.find_connected_components(ConnectedComponentsConfig::default());
    assert_eq!(comps.len(), 2);
    for c in &mut comps {
        c.sort();
    }
    comps.sort_by(|x, y| x[0].cmp(&y[0]));

    let mut expected_comp1 = vec![node_a, node_b];
    expected_comp1.sort();
    let mut expected_comp2 = vec![node_c, node_d];
    expected_comp2.sort();

    let mut expected = vec![expected_comp1, expected_comp2];
    expected.sort_by(|x, y| x[0].cmp(&y[0]));

    assert_eq!(comps, expected);

    // 2. Degree Centrality
    let centrality = engine.calculate_degree_centrality(CentralityConfig::default());
    assert_eq!(centrality.len(), 4);
    // Every node has degree of 1
    for c in &centrality {
        assert_eq!(c.score, 1);
    }

    // 3. Relation Distribution
    let relation_dist = engine.relation_distribution(DistributionConfig::default());
    assert_eq!(relation_dist.len(), 2);
    assert_eq!(relation_dist[0].count, 1);
    assert_eq!(relation_dist[1].count, 1);

    // 4. Provenance Stats
    let stats = engine.provenance_statistics(ProvenanceConfig::default());
    assert_eq!(
        stats,
        ProvenanceStats {
            total_extracted: 2, // default new edge source is Extracted
            total_inferred: 0,
            total_user_authored: 0,
            total_imported: 0,
        }
    );
}

#[test]
fn test_golden_ordering_determinism() {
    let mut graph = KnowledgeGraph::new();

    // Create 4 isolated nodes
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();
    let node_d = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_d, "NodeD".to_string(), NodeType::Concept));

    // Construct exactly identical degrees/scores for all nodes (e.g. 1 edge each)
    // A -uses-> B
    // C -uses-> D
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 0.9))
        .unwrap();
    graph
        .add_edge(Edge::new(node_c, node_d, RelationKind::Uses, 0.8))
        .unwrap();

    let ctx = GraphAnalyticsContext::new(&graph);

    let centrality1 = Centrality::new(&ctx, CentralityConfig::default()).compute();
    let centrality2 = Centrality::new(&ctx, CentralityConfig::default()).compute();

    // The order must be exactly identical across executions (deterministic sorting)
    assert_eq!(centrality1, centrality2);

    // Sort logic contract: sorts descending by score, then lexicographically by NodeId
    // All nodes have score 1, so the result should be sorted strictly by NodeId lexicographically
    let mut expected_nodes = vec![node_a, node_b, node_c, node_d];
    expected_nodes.sort();

    let resulting_nodes: Vec<NodeId> = centrality1.iter().map(|c| c.node).collect();
    assert_eq!(resulting_nodes, expected_nodes);

    // Connected Components ordering determinism
    let comps1 = ConnectedComponents::new(&ctx, ConnectedComponentsConfig::default()).compute();
    let comps2 = ConnectedComponents::new(&ctx, ConnectedComponentsConfig::default()).compute();
    assert_eq!(comps1, comps2);
}

#[test]
fn test_generic_solver_conformance() {
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 0.9))
        .unwrap();

    let ctx = GraphAnalyticsContext::new(&graph);

    // 1. ConnectedComponents
    {
        let solver = ConnectedComponents::new(&ctx, ConnectedComponentsConfig::default());
        assert!(!solver.algorithm_id().is_empty());
        assert_eq!(solver.complexity(), Complexity::Linear);
        let res1 = solver.compute();
        let res2 = solver.compute();
        assert_eq!(res1, res2);
    }

    // 2. Centrality
    {
        let solver = Centrality::new(&ctx, CentralityConfig::default());
        assert!(!solver.algorithm_id().is_empty());
        assert_eq!(solver.complexity(), Complexity::Linear);
        let res1 = solver.compute();
        let res2 = solver.compute();
        assert_eq!(res1, res2);
    }

    // 3. Distribution
    {
        let solver = Distribution::new(&ctx, DistributionConfig::default());
        assert!(!solver.algorithm_id().is_empty());
        assert_eq!(solver.complexity(), Complexity::Linear);
        let res1 = solver.compute();
        let res2 = solver.compute();
        assert_eq!(res1, res2);
    }

    // 4. ProvenanceStatistics
    {
        let solver = ProvenanceStatistics::new(&ctx, ProvenanceConfig::default());
        assert!(!solver.algorithm_id().is_empty());
        assert_eq!(solver.complexity(), Complexity::Linear);
        let res1 = solver.compute();
        let res2 = solver.compute();
        assert_eq!(res1, res2);
    }
}

#[test]
fn test_shortest_path_edge_cases() {
    let mut graph = KnowledgeGraph::new();
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();
    let node_d = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_d, "NodeD".to_string(), NodeType::Concept));

    let registry = create_test_registry();
    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);

    // 1. Disconnected graph
    let path_dis = engine.shortest_path(
        node_a,
        node_b,
        ShortestPathConfig::default(),
        UniformWeightProvider,
    );
    assert_eq!(path_dis, None);

    // 2. Equal-cost paths and deterministic tie-breaking.
    // Paths A -> B -> C and A -> D -> C (both cost 2.0 with UniformWeightProvider)
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_a, node_d, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_d, node_c, RelationKind::Uses, 1.0))
        .unwrap();

    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);
    let path_eq = engine.shortest_path(
        node_a,
        node_c,
        ShortestPathConfig::default(),
        UniformWeightProvider,
    );
    assert!(path_eq.is_some());
    let path = path_eq.unwrap();
    // Tie-breaker selects predecessor node lexicographically. Let's make sure it is deterministic.
    assert_eq!(path[0], node_a);
    assert_eq!(path[2], node_c);

    // 3. Max cost threshold filtering
    let config_cost_limit = ShortestPathConfig {
        max_cost: Some(1.0),
    };
    let path_too_long =
        engine.shortest_path(node_a, node_c, config_cost_limit, UniformWeightProvider);
    assert_eq!(path_too_long, None);
}

#[test]
fn test_cycle_detector_edge_cases() {
    let mut graph = KnowledgeGraph::new();
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));

    let registry = create_test_registry();
    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);
    // DAG initially - no cycles
    assert!(!engine.has_cycles(CycleDetectionConfig::default()));

    // 1. Self loop
    graph
        .add_edge(Edge::new(node_a, node_a, RelationKind::Uses, 1.0))
        .unwrap();
    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);
    let cycles_self = engine.find_cycles(CycleDetectionConfig::default());
    assert_eq!(cycles_self.len(), 1);
    assert_eq!(cycles_self[0], vec![node_a]);

    // 2. Overlapping cycles
    // Add cycle A -> B -> A and B -> C -> B
    let mut graph_overlap = KnowledgeGraph::new();
    graph_overlap.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph_overlap.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph_overlap.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));
    graph_overlap
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 1.0))
        .unwrap();
    graph_overlap
        .add_edge(Edge::new(node_b, node_a, RelationKind::Uses, 1.0))
        .unwrap();
    graph_overlap
        .add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 1.0))
        .unwrap();
    graph_overlap
        .add_edge(Edge::new(node_c, node_b, RelationKind::Uses, 1.0))
        .unwrap();

    let report_overlap = GraphValidator::validate(&graph_overlap, &registry);
    let engine_overlap = GraphQueryEngine::new(&graph_overlap, &report_overlap, &registry);
    let cycles_overlap = engine_overlap.find_cycles(CycleDetectionConfig::default());
    // Expecting A -> B -> A and B -> C -> B as distinct cycles
    assert_eq!(cycles_overlap.len(), 2);
}

#[test]
fn test_pagerank_edge_cases() {
    let registry = create_test_registry();

    // 1. Single node graph
    let mut graph_single = KnowledgeGraph::new();
    let node_a = NodeId::new();
    graph_single.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    let report_single = GraphValidator::validate(&graph_single, &registry);
    let engine_single = GraphQueryEngine::new(&graph_single, &report_single, &registry);
    let pr_single = engine_single.pagerank(PageRankConfig::default());
    assert_eq!(pr_single.len(), 1);
    assert!((pr_single[0].score - 1.0).abs() < 1e-5);

    // 2. Isolated nodes and complete graph
    let mut graph = KnowledgeGraph::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();
    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));

    // Complete graph structure
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_c, node_a, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_b, node_a, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_c, node_b, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_a, node_c, RelationKind::Uses, 1.0))
        .unwrap();

    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);
    let pr_scores = engine.pagerank(PageRankConfig::default());
    assert_eq!(pr_scores.len(), 3);
    // Since it's fully symmetrical/complete, all nodes must have exactly equal scores of 1/3.
    assert!((pr_scores[0].score - 0.333333).abs() < 1e-2);
}

#[test]
fn test_scc_edge_cases() {
    let registry = create_test_registry();

    // 1. Empty graph
    let graph_empty = KnowledgeGraph::new();
    let report_empty = GraphValidator::validate(&graph_empty, &registry);
    let engine_empty = GraphQueryEngine::new(&graph_empty, &report_empty, &registry);
    assert_eq!(
        engine_empty
            .strongly_connected_components(SccConfig::default())
            .len(),
        0
    );

    // 2. Singleton SCCs
    let mut graph = KnowledgeGraph::new();
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 1.0))
        .unwrap();

    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);
    let sccs = engine.strongly_connected_components(SccConfig::default());
    assert_eq!(sccs.len(), 2);
    assert_eq!(sccs[0].nodes.len(), 1);
    assert_eq!(sccs[1].nodes.len(), 1);

    // 3. Nested/overlapping cyclic regions
    // Node A <-> Node B and Node B -> Node C (C has self loop)
    let node_c = NodeId::new();
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));
    graph
        .add_edge(Edge::new(node_b, node_a, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_c, node_c, RelationKind::Uses, 1.0))
        .unwrap();
    let report_nested = GraphValidator::validate(&graph, &registry);
    let engine_nested = GraphQueryEngine::new(&graph, &report_nested, &registry);
    let sccs_nested = engine_nested.strongly_connected_components(SccConfig::default());
    assert_eq!(sccs_nested.len(), 2);
}

#[test]
fn test_shuffle_insertion_determinism() {
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();
    let node_d = NodeId::new();

    let nodes = vec![
        Node::new(node_a, "NodeA".to_string(), NodeType::Concept),
        Node::new(node_b, "NodeB".to_string(), NodeType::Concept),
        Node::new(node_c, "NodeC".to_string(), NodeType::Concept),
        Node::new(node_d, "NodeD".to_string(), NodeType::Concept),
    ];

    let edges = vec![
        Edge::new(node_a, node_b, RelationKind::Uses, 1.0),
        Edge::new(node_b, node_c, RelationKind::Uses, 1.0),
        Edge::new(node_c, node_a, RelationKind::Uses, 1.0),
        Edge::new(node_c, node_d, RelationKind::Uses, 1.0),
        Edge::new(node_d, node_c, RelationKind::Uses, 1.0),
    ];

    // Build Graph 1: Original insertion order
    let mut graph1 = KnowledgeGraph::new();
    for node in &nodes {
        graph1.add_node(node.clone());
    }
    for edge in &edges {
        graph1.add_edge(edge.clone()).unwrap();
    }

    // Build Graph 2: Shuffled order
    let mut graph2 = KnowledgeGraph::new();
    // Shuffled nodes
    graph2.add_node(nodes[3].clone());
    graph2.add_node(nodes[1].clone());
    graph2.add_node(nodes[0].clone());
    graph2.add_node(nodes[2].clone());
    // Shuffled edges
    graph2.add_edge(edges[4].clone()).unwrap();
    graph2.add_edge(edges[2].clone()).unwrap();
    graph2.add_edge(edges[0].clone()).unwrap();
    graph2.add_edge(edges[3].clone()).unwrap();
    graph2.add_edge(edges[1].clone()).unwrap();

    let registry = create_test_registry();
    let report1 = GraphValidator::validate(&graph1, &registry);
    let engine1 = GraphQueryEngine::new(&graph1, &report1, &registry);

    let report2 = GraphValidator::validate(&graph2, &registry);
    let engine2 = GraphQueryEngine::new(&graph2, &report2, &registry);

    // Verify Shortest Path Output Determinism
    let sp1 = engine1.shortest_path(
        node_a,
        node_d,
        ShortestPathConfig::default(),
        UniformWeightProvider,
    );
    let sp2 = engine2.shortest_path(
        node_a,
        node_d,
        ShortestPathConfig::default(),
        UniformWeightProvider,
    );
    assert_eq!(sp1, sp2);

    // Verify Cycle Detection Output Determinism
    let cycles1 = engine1.find_cycles(CycleDetectionConfig::default());
    let cycles2 = engine2.find_cycles(CycleDetectionConfig::default());
    assert_eq!(cycles1, cycles2);

    // Verify PageRank Score Determinism
    let pr1 = engine1.pagerank(PageRankConfig::default());
    let pr2 = engine2.pagerank(PageRankConfig::default());
    assert_eq!(pr1, pr2);

    // Verify Strongly Connected Components Determinism
    let scc1 = engine1.strongly_connected_components(SccConfig::default());
    let scc2 = engine2.strongly_connected_components(SccConfig::default());
    assert_eq!(scc1, scc2);
}

#[test]
fn test_astar_search() {
    let mut graph = KnowledgeGraph::new();
    let node_a = NodeId::new();
    let node_b_raw = NodeId::new();
    let node_c_raw = NodeId::new();
    let (node_b, node_c) = if node_b_raw < node_c_raw {
        (node_b_raw, node_c_raw)
    } else {
        (node_c_raw, node_b_raw)
    };
    let node_d = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_d, "NodeD".to_string(), NodeType::Concept));

    // A -> B (weight 1.0) -> D (weight 1.0)
    // A -> C (weight 2.0) -> D (weight 0.5)
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_b, node_d, RelationKind::Uses, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_a, node_c, RelationKind::Uses, 2.0))
        .unwrap();
    graph
        .add_edge(Edge::new(node_c, node_d, RelationKind::Uses, 0.5))
        .unwrap();

    let registry = create_test_registry();
    let report = GraphValidator::validate(&graph, &registry);
    let engine = GraphQueryEngine::new(&graph, &report, &registry);

    // 1. Zero Heuristic should match Dijkstra
    let path_dijkstra = engine.shortest_path(
        node_a,
        node_d,
        ShortestPathConfig::default(),
        UniformWeightProvider,
    );
    let path_astar_zero = engine.astar_shortest_path(
        node_a,
        node_d,
        AStarConfig::default(),
        UniformWeightProvider,
        ZeroHeuristic,
    );
    assert_eq!(path_dijkstra, path_astar_zero);
    let expected_mid = if node_b < node_c { node_b } else { node_c };
    assert_eq!(path_astar_zero, Some(vec![node_a, expected_mid, node_d])); // uniform path is 2 hops

    // 2. Admissible heuristic
    struct AdmissibleHeuristic {
        target: NodeId,
        node_b: NodeId,
    }
    impl HeuristicProvider for AdmissibleHeuristic {
        fn estimate(&self, from: NodeId, to: NodeId, _ctx: &GraphAnalyticsContext) -> f64 {
            if to == self.target {
                if from == self.node_b {
                    0.5 // admissible estimate (actual is 1.0)
                } else {
                    0.1
                }
            } else {
                0.0
            }
        }
    }
    let path_astar_admissible = engine.astar_shortest_path(
        node_a,
        node_d,
        AStarConfig::default(),
        UniformWeightProvider,
        AdmissibleHeuristic {
            target: node_d,
            node_b,
        },
    );
    assert_eq!(path_astar_admissible, Some(vec![node_a, node_b, node_d]));
}

#[test]
fn test_closeness_centrality_topologies() {
    let registry = create_test_registry();

    // 1. Complete graph K3
    {
        let mut graph = KnowledgeGraph::new();
        let a = NodeId::new();
        let b = NodeId::new();
        let c = NodeId::new();
        graph.add_node(Node::new(a, "A".to_string(), NodeType::Concept));
        graph.add_node(Node::new(b, "B".to_string(), NodeType::Concept));
        graph.add_node(Node::new(c, "C".to_string(), NodeType::Concept));
        graph
            .add_edge(Edge::new(a, b, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(b, a, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(b, c, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(c, b, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(a, c, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(c, a, RelationKind::Uses, 1.0))
            .unwrap();

        let report = GraphValidator::validate(&graph, &registry);
        let engine = GraphQueryEngine::new(&graph, &report, &registry);
        let res = engine.closeness_centrality(
            ClosenessConfig {
                variant: ClosenessVariant::Classic,
            },
            UniformWeightProvider,
        );
        assert_eq!(res.len(), 3);
        // All scores must be 1.0
        for r in res {
            assert!((r.score - 1.0).abs() < 1e-9);
        }
    }

    // 2. Star graph (center = A, leaves = B, C)
    {
        let mut graph = KnowledgeGraph::new();
        let a = NodeId::new();
        let b = NodeId::new();
        let c = NodeId::new();
        graph.add_node(Node::new(a, "A".to_string(), NodeType::Concept));
        graph.add_node(Node::new(b, "B".to_string(), NodeType::Concept));
        graph.add_node(Node::new(c, "C".to_string(), NodeType::Concept));
        graph
            .add_edge(Edge::new(a, b, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(b, a, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(a, c, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(c, a, RelationKind::Uses, 1.0))
            .unwrap();

        let report = GraphValidator::validate(&graph, &registry);
        let engine = GraphQueryEngine::new(&graph, &report, &registry);
        let res = engine.closeness_centrality(
            ClosenessConfig {
                variant: ClosenessVariant::Classic,
            },
            UniformWeightProvider,
        );
        // Center A should have closeness 1.0 (distance 1.0 to both B and C).
        // Leaves B and C should have closeness 2 / (1 + 2) = 2/3 = 0.6666
        let map: std::collections::HashMap<_, _> =
            res.into_iter().map(|r| (r.node, r.score)).collect();
        assert!((map[&a] - 1.0).abs() < 1e-9);
        assert!((map[&b] - 2.0 / 3.0).abs() < 1e-9);
        assert!((map[&c] - 2.0 / 3.0).abs() < 1e-9);
    }

    // 3. Disconnected and isolated node
    {
        let mut graph = KnowledgeGraph::new();
        let a = NodeId::new();
        let b = NodeId::new();
        let c = NodeId::new(); // isolated
        graph.add_node(Node::new(a, "A".to_string(), NodeType::Concept));
        graph.add_node(Node::new(b, "B".to_string(), NodeType::Concept));
        graph.add_node(Node::new(c, "C".to_string(), NodeType::Concept));
        graph
            .add_edge(Edge::new(a, b, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(b, a, RelationKind::Uses, 1.0))
            .unwrap();

        let report = GraphValidator::validate(&graph, &registry);
        let engine = GraphQueryEngine::new(&graph, &report, &registry);

        // Classic should yield 0.0 for all nodes due to unreachability
        let classic = engine.closeness_centrality(
            ClosenessConfig {
                variant: ClosenessVariant::Classic,
            },
            UniformWeightProvider,
        );
        for r in classic {
            assert_eq!(r.score, 0.0);
        }

        // Harmonic closeness should handle isolated node
        let harmonic = engine.closeness_centrality(
            ClosenessConfig {
                variant: ClosenessVariant::Harmonic,
            },
            UniformWeightProvider,
        );
        let map: std::collections::HashMap<_, _> =
            harmonic.into_iter().map(|r| (r.node, r.score)).collect();
        // A has reachability to B (dist 1). Harmonic = (1/1) / 2 = 0.5
        assert!((map[&a] - 0.5).abs() < 1e-9);
        // C has reachability to nothing. Harmonic = 0
        assert_eq!(map[&c], 0.0);
    }
}

#[test]
fn test_connectivity_diagnostics() {
    let registry = create_test_registry();

    // 1. Single node
    {
        let mut graph = KnowledgeGraph::new();
        let a = NodeId::new();
        graph.add_node(Node::new(a, "A".to_string(), NodeType::Concept));

        let report = GraphValidator::validate(&graph, &registry);
        let engine = GraphQueryEngine::new(&graph, &report, &registry);
        let conn = engine.connectivity_diagnostics(ConnectivityConfig::default());
        assert!(conn.articulation_points.is_empty());
        assert!(conn.bridges.is_empty());
    }

    // 2. Tree: A-B-C
    {
        let mut graph = KnowledgeGraph::new();
        let a = NodeId::new();
        let b = NodeId::new();
        let c = NodeId::new();
        graph.add_node(Node::new(a, "A".to_string(), NodeType::Concept));
        graph.add_node(Node::new(b, "B".to_string(), NodeType::Concept));
        graph.add_node(Node::new(c, "C".to_string(), NodeType::Concept));
        graph
            .add_edge(Edge::new(a, b, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(b, c, RelationKind::Uses, 1.0))
            .unwrap();

        let report = GraphValidator::validate(&graph, &registry);
        let engine = GraphQueryEngine::new(&graph, &report, &registry);
        let conn = engine.connectivity_diagnostics(ConnectivityConfig::default());
        // Articulation point is B
        assert_eq!(conn.articulation_points, vec![b]);
        // Bridges are both edges (sorted canonically)
        let mut expected_bridges = vec![
            if a < b { (a, b) } else { (b, a) },
            if b < c { (b, c) } else { (c, b) },
        ];
        expected_bridges.sort_by(|x, y| x.0.cmp(&y.0).then_with(|| x.1.cmp(&y.1)));
        assert_eq!(conn.bridges, expected_bridges);
    }

    // 3. Cycle K3 + bridge to leaf (D)
    // A-B-C-A and C-D
    {
        let mut graph = KnowledgeGraph::new();
        let a = NodeId::new();
        let b = NodeId::new();
        let c = NodeId::new();
        let d = NodeId::new();
        graph.add_node(Node::new(a, "A".to_string(), NodeType::Concept));
        graph.add_node(Node::new(b, "B".to_string(), NodeType::Concept));
        graph.add_node(Node::new(c, "C".to_string(), NodeType::Concept));
        graph.add_node(Node::new(d, "D".to_string(), NodeType::Concept));
        graph
            .add_edge(Edge::new(a, b, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(b, c, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(c, a, RelationKind::Uses, 1.0))
            .unwrap();
        graph
            .add_edge(Edge::new(c, d, RelationKind::Uses, 1.0))
            .unwrap();

        let report = GraphValidator::validate(&graph, &registry);
        let engine = GraphQueryEngine::new(&graph, &report, &registry);
        let conn = engine.connectivity_diagnostics(ConnectivityConfig::default());
        // C is the only articulation point (removing it isolates D)
        assert_eq!(conn.articulation_points, vec![c]);
        // C-D is the only bridge
        let expected_bridge = if c < d { (c, d) } else { (d, c) };
        assert_eq!(conn.bridges, vec![expected_bridge]);
    }
}
