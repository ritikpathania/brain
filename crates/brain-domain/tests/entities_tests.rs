use brain_domain::*;

#[test]
fn test_edge_strengthen_and_decay() {
    let source = NodeId::new();
    let target = NodeId::new();
    let mut edge = Edge::new(source, target, "relates_to".to_string(), 0.5);

    // Test strengthening
    assert!(edge.strengthen().is_ok());
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
    let mut edge2 = Edge::new(source, target, "relates_to".to_string(), 1.0);
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
fn test_conversation_archiving() {
    let mut conv = Conversation::new_empty();
    assert!(!conv.is_archived());

    let msg1 = Message::new(MessageId::new(), MessageRole::User, "Hello".to_string());
    assert!(conv.add_message(msg1).is_ok());
    assert_eq!(conv.messages.len(), 1);

    // Archive it
    assert!(conv.archive().is_ok());
    assert!(conv.is_archived());

    // Archiving again should err
    assert!(conv.archive().is_err());

    // Cannot add message when archived
    let msg2 = Message::new(MessageId::new(), MessageRole::Assistant, "Hi".to_string());
    assert!(conv.add_message(msg2).is_err());
    assert_eq!(conv.messages.len(), 1);
}

#[test]
fn test_knowledge_graph_invariants() {
    let mut kg = KnowledgeGraph::new();

    let node1 = Node::new(NodeId::new(), "Node1".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Node2".to_string(), NodeType::Concept);

    kg.add_node(node1.clone());

    // Try adding edge where target doesn't exist
    let edge1 = Edge::new(node1.id, node2.id, "linked_to".to_string(), 0.5);
    let result = kg.add_edge(edge1);
    assert!(matches!(result, Err(DomainError::MissingTargetNode(_))));

    // Try adding edge where source doesn't exist
    let edge2 = Edge::new(node2.id, node1.id, "linked_to".to_string(), 0.5);
    let result = kg.add_edge(edge2);
    assert!(matches!(result, Err(DomainError::MissingSourceNode(_))));

    // Add node2
    kg.add_node(node2.clone());

    // Add edge successfully
    let edge3 = Edge::new(node1.id, node2.id, "linked_to".to_string(), 0.5);
    assert!(kg.add_edge(edge3).is_ok());

    // Try adding duplicate edge
    let edge4 = Edge::new(node1.id, node2.id, "linked_to".to_string(), 0.8);
    let result = kg.add_edge(edge4);
    assert!(matches!(result, Err(DomainError::EdgeAlreadyExists { .. })));

    // Strengthen existing relationship
    assert!(kg.strengthen_relationship(node1.id, node2.id, "linked_to".to_string()).is_ok());
    let edge_id = EdgeId::new(node1.id, node2.id, "linked_to".to_string());
    assert!((kg.edges.get(&edge_id).unwrap().weight - 0.6).abs() < 1e-9);

    // Strengthen non-existent edge
    let result = kg.strengthen_relationship(node2.id, node1.id, "linked_to".to_string());
    assert!(result.is_err());
}

#[test]
fn test_session_goals() {
    let mut session = Session::new(SessionId::new());
    assert!(session.goals.is_empty());

    // Add goal
    assert!(session.add_goal("Study Rust".to_string()).is_ok());
    assert_eq!(session.goals, vec!["Study Rust".to_string()]);

    // Empty goal should err
    assert!(session.add_goal("   ".to_string()).is_err());

    // Duplicate goal should err
    assert!(session.add_goal("Study Rust".to_string()).is_err());

    // Remove goal
    assert!(session.remove_goal("Study Rust").is_ok());
    assert!(session.goals.is_empty());

    // Remove non-existent goal should err
    assert!(session.remove_goal("Study Rust").is_err());
}
