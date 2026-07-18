use brain_domain::*;

#[test]
fn test_edge_strengthen_and_decay() {
    let source = NodeId::new();
    let target = NodeId::new();
    let mut edge = Edge::new(source, target, RelationKind::AssociatedWith, 0.5);

    // Test strengthening
    let res = edge.strengthen();
    assert!(res.is_ok());
    let event = res.unwrap();
    assert!(
        matches!(event, DomainEvent::RelationshipStrengthened { source: ref s, target: ref t, relation: ref r, new_weight } 
        if s == &edge.source.to_string() && t == &edge.target.to_string() && r == "associated_with" && (new_weight - 0.6).abs() < 1e-9)
    );
    assert!((edge.weight - 0.6).abs() < 1e-9);

    // Test strengthen capped at 1.0
    edge.weight = 0.95;
    assert!(edge.strengthen().is_ok());
    assert!((edge.weight - 1.0).abs() < 1e-9);

    // Test strengthen on invalid weight
    edge.weight = -0.1;
    assert!(edge.strengthen().is_err());
    edge.weight = 1.1;
    assert!(edge.strengthen().is_err());

    // Test decay
    let mut edge2 = Edge::new(source, target, RelationKind::AssociatedWith, 1.0);
    // Half life: 10s, delta_t: 10s -> weight should become 0.5
    assert!(edge2.decay(10.0, 10.0).is_ok());
    assert!((edge2.weight - 0.5).abs() < 1e-9);

    // Test decay with invalid half life
    assert!(edge2.decay(0.0, 5.0).is_err());
    assert!(edge2.decay(-1.0, 5.0).is_err());
    // Test decay with invalid delta_t
    assert!(edge2.decay(10.0, -1.0).is_err());
}

#[test]
fn test_session_archiving() {
    let id = SessionId::new();
    let title = SessionTitle("Test Session".to_string());
    let timestamp = SessionTimestamp(0);
    let mut session = Session::new(id, title, timestamp);
    assert!(!session.archived);

    let msg1 = Message::new(MessageId::new(), MessageRole::User, "Hello".to_string());
    assert!(session.add_message(msg1).is_ok());
    assert_eq!(session.messages.len(), 1);

    // Archive it
    let res = session.archive(SessionTimestamp(10));
    assert!(res.is_ok());

    // Check events
    let events: Vec<_> = session.drain_events().collect();
    // 0 is SessionCreated, 1 is MessageAdded, 2 is SessionArchived
    assert_eq!(events.len(), 3);
    assert!(
        matches!(events[2], DomainEvent::SessionArchived { session_id, updated_at }
        if session_id == id && updated_at == SessionTimestamp(10))
    );
    assert!(session.archived);

    // Archiving again should err
    assert!(session.archive(SessionTimestamp(20)).is_err());

    // Cannot add message when archived
    let msg2 = Message::new(MessageId::new(), MessageRole::Assistant, "Hi".to_string());
    assert!(session.add_message(msg2).is_err());
    assert_eq!(session.messages.len(), 1);
}

#[test]
fn test_knowledge_graph_invariants() {
    let mut kg = KnowledgeGraph::new();

    let node1 = Node::new(NodeId::new(), "Node1".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Node2".to_string(), NodeType::Concept);

    kg.add_node(node1.clone());

    // Try adding edge where target doesn't exist
    let edge1 = Edge::new(node1.id, node2.id, RelationKind::AssociatedWith, 0.5);
    let result = kg.add_edge(edge1);
    assert!(matches!(result, Err(DomainError::MissingTargetNode(_))));

    // Try adding edge where source doesn't exist
    let edge2 = Edge::new(node2.id, node1.id, RelationKind::AssociatedWith, 0.5);
    let result = kg.add_edge(edge2);
    assert!(matches!(result, Err(DomainError::MissingSourceNode(_))));

    // Add node2
    kg.add_node(node2.clone());

    // Add edge successfully
    let edge3 = Edge::new(node1.id, node2.id, RelationKind::AssociatedWith, 0.5);
    assert!(kg.add_edge(edge3).is_ok());

    // Try adding duplicate edge
    let edge4 = Edge::new(node1.id, node2.id, RelationKind::AssociatedWith, 0.8);
    let result = kg.add_edge(edge4);
    assert!(matches!(result, Err(DomainError::EdgeAlreadyExists { .. })));

    // Strengthen existing relationship
    let res = kg.strengthen_relationship(node1.id, node2.id, RelationId::new("associated_with"));
    assert!(res.is_ok());
    let event = res.unwrap();
    assert!(
        matches!(event, DomainEvent::RelationshipStrengthened { ref source, ref target, ref relation, new_weight }
        if source == &node1.id.to_string() && target == &node2.id.to_string() && relation == "associated_with" && (new_weight - 0.6).abs() < 1e-9)
    );

    let edge_id = EdgeId::new(node1.id, node2.id, RelationId::new("associated_with"));
    assert!((kg.edges.get(&edge_id).unwrap().weight - 0.6).abs() < 1e-9);

    // Strengthen non-existent edge
    let result = kg.strengthen_relationship(node2.id, node1.id, RelationId::new("associated_with"));
    assert!(result.is_err());
}

#[test]
fn test_session_goals() {
    let id = SessionId::new();
    let title = SessionTitle("Test Session".to_string());
    let timestamp = SessionTimestamp(0);
    let mut session = Session::new(id, title, timestamp);
    assert!(session.goals.is_empty());

    // Add goal
    let goal_id = GoalId::new();
    let goal = Goal {
        id: goal_id,
        text: "Study Rust".to_string(),
    };
    assert!(session.add_goal(goal.clone()).is_ok());
    assert_eq!(session.goals, vec![goal]);

    // Empty goal should err
    let empty_goal = Goal {
        id: GoalId::new(),
        text: "   ".to_string(),
    };
    assert!(session.add_goal(empty_goal).is_err());

    // Duplicate goal should err
    let dup_goal = Goal {
        id: GoalId::new(),
        text: "Study Rust".to_string(),
    };
    assert!(session.add_goal(dup_goal).is_err());

    // Remove goal
    assert!(session.remove_goal(&goal_id).is_ok());
    assert!(session.goals.is_empty());

    // Remove non-existent goal should err
    assert!(session.remove_goal(&goal_id).is_err());
}

#[test]
fn test_graph_builder_validation() {
    let defs = vec![RelationDefinition {
        id: RelationId::new("uses"),
        display_name: "uses".to_string(),
        inverse: None,
        directionality: Directionality::Directed,
        symmetry: false,
        transitivity: false,
        fallback_suppression: false,
        confidence_strategy: ConfidenceStrategy::SourceDefined,
        description: "Test".to_string(),
    }];
    let registry = RelationRegistry::new(defs).unwrap();

    let node1 = Node::new(NodeId::new(), "Node1".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Node2".to_string(), NodeType::Concept);

    let builder = GraphBuilder::new(&registry)
        .add_node(node1.clone())
        .add_node(node2.clone());

    let res = builder.add_edge(node1.id, node2.id, RelationKind::Uses, 0.8);
    assert!(res.is_ok());
    let graph = res.unwrap().build();
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.edges.len(), 1);

    let builder2 = GraphBuilder::new(&registry)
        .add_node(node1.clone())
        .add_node(node2.clone());
    let res2 = builder2.add_edge(node1.id, node2.id, RelationKind::DependsOn, 0.8);
    assert!(matches!(res2, Err(DomainError::UnregisteredRelation(_))));
}

#[test]
fn test_normalizer_idempotence() {
    let cases = vec![
        " Postgres ",
        "postgres",
        "  POSTGRES  ",
        "post-gres",
        "ReactJS",
        "  TrimMe  ",
    ];
    for s in cases {
        let pass1 = brain_domain::Normalizer::normalize(s);
        let pass2 = brain_domain::Normalizer::normalize(&pass1);
        assert_eq!(pass1, pass2, "Idempotency failed for: {}", s);
    }
}

#[test]
fn test_canonicalization_order_determinism() {
    let registry = RelationRegistry::default_embedded();

    let labels_permutation_1 = vec!["Postgres", "postgres", "  POSTGRES  "];
    let labels_permutation_2 = vec!["  POSTGRES  ", "Postgres", "postgres"];

    let canonical_id = NodeId::new();
    let mut mappings = std::collections::HashMap::new();
    mappings.insert("postgres".to_string(), canonical_id);
    let resolver = brain_domain::AliasResolver::new(mappings);
    let canonicalizer = brain_domain::EntityCanonicalizer::new(resolver);

    let mut builder1 = GraphBuilder::new(&registry).with_canonicalizer(canonicalizer.clone());
    for label in &labels_permutation_1 {
        let n = Node::new(NodeId::new(), label.to_string(), NodeType::Concept);
        builder1 = builder1.add_node(n);
    }
    let graph1 = builder1.build();

    let mut builder2 = GraphBuilder::new(&registry).with_canonicalizer(canonicalizer.clone());
    for label in &labels_permutation_2 {
        let n = Node::new(NodeId::new(), label.to_string(), NodeType::Concept);
        builder2 = builder2.add_node(n);
    }
    let graph2 = builder2.build();

    assert_eq!(graph1.nodes.len(), 1);
    assert_eq!(graph2.nodes.len(), 1);

    let node1 = graph1.nodes.get(&canonical_id).expect("Node not found");
    let node2 = graph2.nodes.get(&canonical_id).expect("Node not found");

    assert_eq!(node1.label, "postgres");
    assert_eq!(node2.label, "postgres");

    assert_eq!(node1.id, node2.id);
    assert_eq!(node1.label, node2.label);
    assert_eq!(node1.node_type, node2.node_type);
}

#[test]
fn test_alias_equivalency() {
    let registry = RelationRegistry::default_embedded();

    let canonical_id = NodeId::new();
    let mut mappings = std::collections::HashMap::new();
    mappings.insert("postgres".to_string(), canonical_id);
    mappings.insert("postgresql".to_string(), canonical_id);
    mappings.insert("pgsql".to_string(), canonical_id);
    let resolver = brain_domain::AliasResolver::new(mappings);
    let canonicalizer = brain_domain::EntityCanonicalizer::new(resolver);

    let node_other_id = NodeId::new();
    let node_other = Node::new(node_other_id, "NodeOther".to_string(), NodeType::Concept);

    let mut builder_pgsql = GraphBuilder::new(&registry)
        .with_canonicalizer(canonicalizer.clone())
        .add_node(Node::new(
            NodeId::new(),
            "pgsql".to_string(),
            NodeType::Concept,
        ))
        .add_node(node_other.clone());
    builder_pgsql = builder_pgsql
        .add_edge(canonical_id, node_other_id, RelationKind::Uses, 1.0)
        .unwrap();
    let graph_pgsql = builder_pgsql.build();

    let mut builder_psql = GraphBuilder::new(&registry)
        .with_canonicalizer(canonicalizer.clone())
        .add_node(Node::new(
            NodeId::new(),
            "postgresql".to_string(),
            NodeType::Concept,
        ))
        .add_node(node_other.clone());
    builder_psql = builder_psql
        .add_edge(canonical_id, node_other_id, RelationKind::Uses, 1.0)
        .unwrap();
    let graph_psql = builder_psql.build();

    assert_eq!(graph_pgsql.nodes.len(), 2);
    assert_eq!(graph_psql.nodes.len(), 2);
    assert_eq!(graph_pgsql.edges.len(), 1);
    assert_eq!(graph_psql.edges.len(), 1);

    let edge_id = EdgeId::new(canonical_id, node_other_id, RelationKind::Uses.id());
    let edge_pgsql = graph_pgsql.edges.get(&edge_id).unwrap();
    let edge_psql = graph_psql.edges.get(&edge_id).unwrap();

    assert_eq!(edge_pgsql.source, edge_psql.source);
    assert_eq!(edge_pgsql.target, edge_psql.target);
    assert_eq!(edge_pgsql.relation, edge_psql.relation);
    assert_eq!(edge_pgsql.weight, edge_psql.weight);
}
