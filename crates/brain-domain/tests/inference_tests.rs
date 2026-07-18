use brain_domain::{
    ConfidenceStrategy, Directionality, Edge, EdgeId, InferenceEngine, KnowledgeGraph, Node,
    NodeId, NodeType, ProvenanceSource, RelationDefinition, RelationId, RelationKind,
    RelationRegistry, SuppressionEngine,
};

fn create_test_registry() -> RelationRegistry {
    // We define custom relations to test all parts of the inference/suppression systems.
    let defs = vec![
        RelationDefinition {
            id: RelationId::new("uses"),
            display_name: "uses".to_string(),
            inverse: None,
            directionality: Directionality::Directed,
            symmetry: false,
            transitivity: true,
            fallback_suppression: true,
            confidence_strategy: ConfidenceStrategy::Maximum,
            description: "Transitive uses".to_string(),
        },
        RelationDefinition {
            id: RelationId::new("develops"),
            display_name: "develops".to_string(),
            inverse: Some(RelationId::new("stored_in")),
            directionality: Directionality::Directed,
            symmetry: false,
            transitivity: false,
            fallback_suppression: true,
            confidence_strategy: ConfidenceStrategy::SourceDefined,
            description: "Develops owned by repository".to_string(),
        },
        RelationDefinition {
            id: RelationId::new("stored_in"),
            display_name: "stored_in".to_string(),
            inverse: Some(RelationId::new("develops")),
            directionality: Directionality::Directed,
            symmetry: false,
            transitivity: false,
            fallback_suppression: true,
            confidence_strategy: ConfidenceStrategy::SourceDefined,
            description: "Stored in repository".to_string(),
        },
        RelationDefinition {
            id: RelationId::new("associated_with"),
            display_name: "associated_with".to_string(),
            inverse: Some(RelationId::new("associated_with")),
            directionality: Directionality::Undirected,
            symmetry: true,
            transitivity: false,
            fallback_suppression: false,
            confidence_strategy: ConfidenceStrategy::Average,
            description: "Generic symmetric association".to_string(),
        },
    ];
    RelationRegistry::new(defs).expect("Failed to build registry")
}

#[test]
fn test_inference_inverse_relation() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));

    // A -develops-> B
    let edge = Edge::new(node_a, node_b, RelationKind::Develops, 0.8);
    graph.add_edge(edge).unwrap();

    let inferred_edges = InferenceEngine::infer(&graph, &registry);

    // Should infer 1 edge: B -stored_in-> A
    assert_eq!(inferred_edges.len(), 1);
    let inf = &inferred_edges[0];
    assert_eq!(inf.source, node_b);
    assert_eq!(inf.target, node_a);
    assert_eq!(inf.relation, RelationKind::StoredIn);
    assert_eq!(inf.weight, 0.8);

    // Provenance Monotonicity: Inferred provenance source must be ProvenanceSource::Inferred
    assert_eq!(inf.provenance.source, ProvenanceSource::Inferred);

    let mut graph_final = graph.clone();
    for e in inferred_edges {
        graph_final.add_edge(e).unwrap();
    }
    assert_explanation_completeness(&graph_final);
}

#[test]
fn test_inference_transitive_fixed_point() {
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

    // A -uses-> B (0.9), B -uses-> C (0.8), C -uses-> D (0.7)
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 0.9))
        .unwrap();
    graph
        .add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 0.8))
        .unwrap();
    graph
        .add_edge(Edge::new(node_c, node_d, RelationKind::Uses, 0.7))
        .unwrap();

    let inferred_edges = InferenceEngine::infer(&graph, &registry);

    // Transitive closure on uses (strategy: Maximum):
    // Paths derived:
    // A -uses-> C (max(0.9, 0.8) = 0.9)
    // B -uses-> D (max(0.8, 0.7) = 0.8)
    // A -uses-> D (max(A->C, C->D) = max(0.9, 0.7) = 0.9)
    assert_eq!(inferred_edges.len(), 3);

    // Check we have A->C, B->D, A->D
    let mut has_ac = false;
    let mut has_bd = false;
    let mut has_ad = false;

    for edge in &inferred_edges {
        assert_eq!(edge.provenance.source, ProvenanceSource::Inferred);
        if edge.source == node_a && edge.target == node_c {
            assert_eq!(edge.weight, 0.9);
            has_ac = true;
        } else if edge.source == node_b && edge.target == node_d {
            assert_eq!(edge.weight, 0.8);
            has_bd = true;
        } else if edge.source == node_a && edge.target == node_d {
            assert_eq!(edge.weight, 0.9);
            has_ad = true;
        }
    }

    assert!(has_ac);
    assert!(has_bd);
    assert!(has_ad);

    let mut graph_final = graph.clone();
    for edge in inferred_edges {
        graph_final.add_edge(edge).unwrap();
    }
    assert_explanation_completeness(&graph_final);
}

#[test]
fn test_inference_idempotence_and_determinism() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));

    // A -uses-> B (0.9), B -uses-> C (0.8)
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 0.9))
        .unwrap();
    graph
        .add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 0.8))
        .unwrap();

    let pass_1 = InferenceEngine::infer(&graph, &registry);

    // Apply derived pass_1 edges into the graph
    let mut graph_with_pass_1 = graph.clone();
    for edge in &pass_1 {
        graph_with_pass_1.add_edge(edge.clone()).unwrap();
    }

    let pass_2 = InferenceEngine::infer(&graph_with_pass_1, &registry);

    // pass_2 must be completely empty since pass_1 reached the fixed point!
    assert!(
        pass_2.is_empty(),
        "Inference is not idempotent! Found new derived edges: {:?}",
        pass_2
    );
}

#[test]
fn test_suppression_data_driven() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));

    // Insert fallback relation (associated_with: fallback_suppression == false)
    // Insert specific relation (develops: fallback_suppression == true)
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::AssociatedWith, 0.5))
        .unwrap();
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Develops, 0.8))
        .unwrap();

    let graph_suppressed = SuppressionEngine::apply_suppression(graph, &registry);

    // The associated_with edge must be filtered out/suppressed, leaving only develops!
    assert_eq!(graph_suppressed.edges.len(), 1);
    let remaining_edge = graph_suppressed.edges.values().next().unwrap();
    assert_eq!(remaining_edge.relation, RelationKind::Develops);
    assert_eq!(remaining_edge.weight, 0.8);
}

#[test]
fn test_inference_explainability_recursive() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));

    let edge1 = Edge::new(node_a, node_b, RelationKind::Uses, 0.9);
    let edge2 = Edge::new(node_b, node_c, RelationKind::Uses, 0.8);
    let edge1_id = EdgeId::new(node_a, node_b, RelationKind::Uses.id());
    let edge2_id = EdgeId::new(node_b, node_c, RelationKind::Uses.id());

    graph.add_edge(edge1).unwrap();
    graph.add_edge(edge2).unwrap();

    let inferred = InferenceEngine::infer(&graph, &registry);
    for e in inferred {
        graph.add_edge(e).unwrap();
    }

    let trans_id = EdgeId::new(node_a, node_c, RelationKind::Uses.id());
    let explanation = graph
        .explain_edge(&trans_id)
        .expect("Explanation should exist");

    assert_eq!(explanation.rule, Some(brain_domain::RuleId::Transitive));
    assert_eq!(explanation.supporting_chains.len(), 2);

    let sub_edge_1 = &explanation.supporting_chains[0];
    let sub_edge_2 = &explanation.supporting_chains[1];
    assert_eq!(sub_edge_1.rule, None);
    assert_eq!(sub_edge_2.rule, None);

    let mut expected_supporting = vec![edge1_id, edge2_id];
    expected_supporting.sort();

    // In EdgeId implementation, it has target and source fields.
    // Let's build their IDs using EdgeId::new(source, target, relation)
    let id1 = EdgeId::new(
        sub_edge_1.edge.source,
        sub_edge_1.edge.target,
        sub_edge_1.edge.relation.id(),
    );
    let id2 = EdgeId::new(
        sub_edge_2.edge.source,
        sub_edge_2.edge.target,
        sub_edge_2.edge.relation.id(),
    );

    let mut actual_supporting = vec![id1, id2];
    actual_supporting.sort();

    assert_eq!(actual_supporting, expected_supporting);
    assert_explanation_completeness(&graph);
}

#[test]
fn test_derivation_determinism_invariant() {
    let registry = create_test_registry();

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();
    let node_d = NodeId::new();

    let mut graph = KnowledgeGraph::new();
    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_d, "NodeD".to_string(), NodeType::Concept));

    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 0.9))
        .unwrap();
    graph
        .add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 0.8))
        .unwrap();
    graph
        .add_edge(Edge::new(node_a, node_d, RelationKind::Uses, 0.9))
        .unwrap();
    graph
        .add_edge(Edge::new(node_d, node_c, RelationKind::Uses, 0.9))
        .unwrap();

    let inferred = InferenceEngine::infer(&graph, &registry);
    let mut graph_final = graph.clone();
    for e in inferred {
        graph_final.add_edge(e).unwrap();
    }

    let trans_id = EdgeId::new(node_a, node_c, RelationKind::Uses.id());
    let derived_edge = graph_final.edges.get(&trans_id).unwrap();

    let deriv = derived_edge.derivation.as_ref().unwrap();
    assert_eq!(deriv.rule, brain_domain::RuleId::Transitive);

    let path_b = {
        let mut v = vec![
            EdgeId::new(node_a, node_b, RelationKind::Uses.id()),
            EdgeId::new(node_b, node_c, RelationKind::Uses.id()),
        ];
        v.sort();
        v
    };
    let path_d = {
        let mut v = vec![
            EdgeId::new(node_a, node_d, RelationKind::Uses.id()),
            EdgeId::new(node_d, node_c, RelationKind::Uses.id()),
        ];
        v.sort();
        v
    };

    let expected_path = if path_b < path_d { path_b } else { path_d };
    assert_eq!(deriv.supporting_edges, expected_path);
    assert_explanation_completeness(&graph_final);
}

fn assert_explanation_completeness(graph: &KnowledgeGraph) {
    for (edge_id, edge) in &graph.edges {
        if edge.provenance.source == ProvenanceSource::Inferred {
            let deriv = edge
                .derivation
                .as_ref()
                .expect("Inferred edge must have derivation record");
            for sup_id in &deriv.supporting_edges {
                assert!(
                    graph.edges.contains_key(sup_id),
                    "Supporting edge {} for inferred edge {} does not exist in the graph",
                    sup_id,
                    edge_id
                );
            }
        }
    }
}
