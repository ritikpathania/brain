use brain_domain::{
    AffectedElement, Derivation, DiagnosticCategory, DiagnosticSeverity, Edge, EdgeId,
    GraphValidator, KnowledgeGraph, Node, NodeId, NodeType, ProvenanceSource, RelationKind,
    RelationRegistry, RuleId,
};

fn create_test_registry() -> RelationRegistry {
    RelationRegistry::default_embedded()
}

#[test]
fn test_validator_pass_identity_missing_nodes() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    // Add an edge without adding the nodes first
    let edge = Edge::new(node_a, node_b, RelationKind::Uses, 0.9);
    let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());
    graph.edges.insert(edge_id.clone(), edge);

    let report = GraphValidator::validate(&graph, &registry);
    let diagnostics = report.diagnostics;

    // Should generate VAL-001 (missing source node) and VAL-002 (missing target node)
    assert_eq!(diagnostics.len(), 2);

    assert_eq!(diagnostics[0].code, "VAL-001");
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostics[0].category,
        DiagnosticCategory::ReferentialIntegrity
    );
    assert!(diagnostics[0]
        .affected
        .contains(&AffectedElement::Node(node_a)));
    assert!(diagnostics[0]
        .affected
        .contains(&AffectedElement::Edge(edge_id.clone())));

    assert_eq!(diagnostics[1].code, "VAL-002");
    assert_eq!(diagnostics[1].severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostics[1].category,
        DiagnosticCategory::ReferentialIntegrity
    );
    assert!(diagnostics[1]
        .affected
        .contains(&AffectedElement::Node(node_b)));
    assert!(diagnostics[1]
        .affected
        .contains(&AffectedElement::Edge(edge_id)));
}

#[test]
fn test_validator_pass_registry_unknown_relation() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));

    // Unknown RelationKind
    let edge = Edge::new(node_a, node_b, RelationKind::Unknown, 0.9);
    let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());
    graph.edges.insert(edge_id.clone(), edge);

    let report = GraphValidator::validate(&graph, &registry);
    let diagnostics = report.diagnostics;

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "VAL-003");
    assert_eq!(
        diagnostics[0].category,
        DiagnosticCategory::RegistryConformance
    );
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert!(diagnostics[0]
        .affected
        .contains(&AffectedElement::Edge(edge_id)));
}

#[test]
fn test_validator_pass_structural_redundancy_and_self_loops() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));

    // Self-loop
    let loop_edge = Edge::new(node_a, node_a, RelationKind::Uses, 0.9);
    let loop_id = EdgeId::new(loop_edge.source, loop_edge.target, loop_edge.relation.id());
    graph.edges.insert(loop_id.clone(), loop_edge);

    let report = GraphValidator::validate(&graph, &registry);
    let diagnostics = report.diagnostics;

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "VAL-005");
    assert_eq!(
        diagnostics[0].category,
        DiagnosticCategory::ReferentialIntegrity
    );
    assert!(diagnostics[0]
        .affected
        .contains(&AffectedElement::Edge(loop_id)));
    assert!(diagnostics[0]
        .affected
        .contains(&AffectedElement::Node(node_a)));
}

#[test]
fn test_validator_pass_explanation_completeness_and_acyclicity() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));

    // Inferred edge with missing derivation
    let mut edge1 = Edge::new(node_a, node_b, RelationKind::Uses, 0.9);
    edge1.provenance.source = ProvenanceSource::Inferred;
    let edge1_id = EdgeId::new(edge1.source, edge1.target, edge1.relation.id());
    graph.edges.insert(edge1_id.clone(), edge1);

    // Inferred edge with missing supporting edges
    let mut edge2 = Edge::new(node_b, node_c, RelationKind::Uses, 0.8);
    edge2.provenance.source = ProvenanceSource::Inferred;
    let non_existent_supporting_id = EdgeId::new(node_a, node_c, RelationKind::StoredIn.id());
    edge2.derivation = Some(Derivation {
        rule: RuleId::Transitive,
        supporting_edges: vec![non_existent_supporting_id.clone()],
    });
    let edge2_id = EdgeId::new(edge2.source, edge2.target, edge2.relation.id());
    graph.edges.insert(edge2_id.clone(), edge2);

    let report = GraphValidator::validate(&graph, &registry);
    let diagnostics = report.diagnostics;

    // Check VAL-006 (missing derivation) and VAL-007 (missing supporting edge)
    let codes: Vec<String> = diagnostics.iter().map(|d| d.code.clone()).collect();
    assert!(codes.contains(&"VAL-006".to_string()));
    assert!(codes.contains(&"VAL-007".to_string()));
}

#[test]
fn test_validator_pass_explanation_cyclicity() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));

    // Circular derivation: edge1 derived from edge2, and edge2 derived from edge1
    let edge1_id = EdgeId::new(node_a, node_b, RelationKind::Uses.id());
    let edge2_id = EdgeId::new(node_b, node_a, RelationKind::Uses.id());

    let mut edge1 = Edge::new(node_a, node_b, RelationKind::Uses, 0.9);
    edge1.provenance.source = ProvenanceSource::Inferred;
    edge1.derivation = Some(Derivation {
        rule: RuleId::Inverse,
        supporting_edges: vec![edge2_id.clone()],
    });

    let mut edge2 = Edge::new(node_b, node_a, RelationKind::Uses, 0.9);
    edge2.provenance.source = ProvenanceSource::Inferred;
    edge2.derivation = Some(Derivation {
        rule: RuleId::Inverse,
        supporting_edges: vec![edge1_id.clone()],
    });

    graph.edges.insert(edge1_id, edge1);
    graph.edges.insert(edge2_id, edge2);

    let report = GraphValidator::validate(&graph, &registry);
    let diagnostics = report.diagnostics;

    // VAL-009 should be triggered due to circular reasoning
    let has_cycle_diagnostic = diagnostics.iter().any(|d| d.code == "VAL-009");
    assert!(
        has_cycle_diagnostic,
        "Circular reasoning VAL-009 diagnostic not triggered. Diagnostics: {:?}",
        diagnostics
    );
}

#[test]
fn test_validation_report_metrics_and_snapshot() {
    let registry = create_test_registry();
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));

    // 1 edge in graph
    graph
        .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 0.9))
        .unwrap();

    let report = GraphValidator::validate(&graph, &registry);

    assert!(report.summary.is_valid);
    assert_eq!(report.summary.total_errors, 0);
    assert_eq!(report.summary.total_warnings, 0);

    // Verify metrics exist for all 4 passes
    assert_eq!(report.metrics.len(), 4);

    for metric in &report.metrics {
        // Since we have 1 edge in graph, every pass should inspect exactly 1 element
        assert_eq!(metric.elements_inspected, 1);
        assert_eq!(metric.diagnostic_count, 0);
        assert!(metric.duration_ms >= 0.0);
    }
}
