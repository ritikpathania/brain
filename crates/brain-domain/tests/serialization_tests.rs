use brain_domain::*;
use std::collections::HashSet;
use std::thread;
use std::time::Duration;

#[test]
fn test_identifier_copy_clone() {
    // Assert Copy trait for Copy IDs
    let id1 = SessionId::new();
    let id2 = id1; // Copy
    assert_eq!(id1, id2);

    let run_id1 = RunId::new();
    let run_id2 = run_id1; // Copy
    assert_eq!(run_id1, run_id2);

    let node_id1 = NodeId::new();
    let node_id2 = node_id1; // Copy
    assert_eq!(node_id1, node_id2);

    let plugin_id1 = PluginId::new();
    let plugin_id2 = plugin_id1; // Copy
    assert_eq!(plugin_id1, plugin_id2);

    let conv_id1 = ConversationId::new();
    let conv_id2 = conv_id1; // Copy
    assert_eq!(conv_id1, conv_id2);

    let msg_id1 = MessageId::new();
    let msg_id2 = msg_id1; // Copy
    assert_eq!(msg_id1, msg_id2);

    let doc_id1 = DocumentId::new();
    let doc_id2 = doc_id1; // Copy
    assert_eq!(doc_id1, doc_id2);

    // Assert Clone trait for EdgeId (non-Copy due to relation String)
    let edge_id1 = EdgeId::new(node_id1, NodeId::new(), "knows".to_string());
    #[allow(clippy::clone_on_copy)]
    let edge_id2 = edge_id1.clone();
    assert_eq!(edge_id1, edge_id2);
}

#[test]
fn test_identifier_equality_and_hashing() {
    let id1 = SessionId::new();
    let id2 = SessionId::new();

    // Equality
    assert_eq!(id1, id1);
    assert_ne!(id1, id2);

    // Hashing / Set insertion
    let mut set = HashSet::new();
    assert!(set.insert(id1));
    assert!(!set.insert(id1)); // Duplicate insertion
    assert!(set.insert(id2));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_uuid_uniqueness() {
    let mut uuids = HashSet::new();
    for _ in 0..1000 {
        let id = NodeId::new();
        assert!(uuids.insert(id), "Duplicate NodeId generated!");
    }
}

#[test]
fn test_ulid_chronological_ordering() {
    let id_old = SessionId::new();
    // Sleep briefly to ensure clock ticks for next ULID generation
    thread::sleep(Duration::from_millis(5));
    let id_new = SessionId::new();

    // Newer ULIDs compare greater than older ones
    assert!(id_new.0 > id_old.0);
}

#[test]
fn test_json_roundtrip_identifiers() {
    let original = SessionId::new();
    let serialized = serde_json::to_string(&original).unwrap();
    let deserialized: SessionId = serde_json::from_str(&serialized).unwrap();
    assert_eq!(original, deserialized);

    let node_id = NodeId::new();
    let serialized_node = serde_json::to_string(&node_id).unwrap();
    let deserialized_node: NodeId = serde_json::from_str(&serialized_node).unwrap();
    assert_eq!(node_id, deserialized_node);

    let edge_id = EdgeId::new(node_id, NodeId::new(), "related_to".to_string());
    let serialized_edge = serde_json::to_string(&edge_id).unwrap();
    let deserialized_edge: EdgeId = serde_json::from_str(&serialized_edge).unwrap();
    assert_eq!(edge_id, deserialized_edge);
}

#[test]
fn test_json_roundtrip_entities() {
    // 1. Node Roundtrip
    let node_id = NodeId::new();
    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "key".to_string(),
        serde_json::Value::String("val".to_string()),
    );

    let original_node = Node::new(node_id, "Sample Node".to_string(), NodeType::Concept)
        .with_properties(properties);

    let serialized = serde_json::to_string(&original_node).unwrap();
    let deserialized: Node = serde_json::from_str(&serialized).unwrap();

    assert_eq!(original_node.id, deserialized.id);
    assert_eq!(original_node.label, deserialized.label);
    assert_eq!(original_node.node_type, deserialized.node_type);
    assert_eq!(original_node.properties, deserialized.properties);
    assert_eq!(original_node.updated_at, deserialized.updated_at);

    // 2. Edge Roundtrip
    let source_id = NodeId::new();
    let target_id = NodeId::new();
    let original_edge = Edge::new(source_id, target_id, "influences".to_string(), 0.85);

    let serialized_edge = serde_json::to_string(&original_edge).unwrap();
    let deserialized_edge: Edge = serde_json::from_str(&serialized_edge).unwrap();

    assert_eq!(original_edge.source, deserialized_edge.source);
    assert_eq!(original_edge.target, deserialized_edge.target);
    assert_eq!(original_edge.relation, deserialized_edge.relation);
    assert_eq!(original_edge.weight, deserialized_edge.weight);
    assert_eq!(original_edge.updated_at, deserialized_edge.updated_at);

    // 3. Embedding Roundtrip
    let original_embedding = Embedding::new(node_id, vec![0.1, -0.5, 0.9]);
    let serialized_embedding = serde_json::to_string(&original_embedding).unwrap();
    let deserialized_embedding: Embedding = serde_json::from_str(&serialized_embedding).unwrap();

    assert_eq!(original_embedding.node_id, deserialized_embedding.node_id);
    assert_eq!(original_embedding.vector, deserialized_embedding.vector);
    assert_eq!(
        original_embedding.dimension,
        deserialized_embedding.dimension
    );

    // 4. Conversation Roundtrip
    let conv_id = ConversationId::new();
    let msg1 = Message::new(MessageId::new(), MessageRole::User, "hello".to_string());
    let msg2 = Message::new(
        MessageId::new(),
        MessageRole::Assistant,
        "hi there".to_string(),
    );

    let original_conv = Conversation::new(conv_id)
        .with_messages(vec![msg1, msg2])
        .with_metadata(
            vec![("session".to_string(), "test".to_string())]
                .into_iter()
                .collect(),
        );

    let serialized_conv = serde_json::to_string(&original_conv).unwrap();
    let deserialized_conv: Conversation = serde_json::from_str(&serialized_conv).unwrap();

    assert_eq!(original_conv.id, deserialized_conv.id);
    assert_eq!(
        original_conv.messages.len(),
        deserialized_conv.messages.len()
    );
    assert_eq!(
        original_conv.messages[0].content,
        deserialized_conv.messages[0].content
    );
    assert_eq!(original_conv.metadata, deserialized_conv.metadata);
}

#[test]
fn test_json_roundtrip_dtos() {
    let node_dto = NodeDTO::new(
        "uuid-string".to_string(),
        "Label".to_string(),
        "Type".to_string(),
        serde_json::Value::Null,
    );
    let edge_dto = EdgeDTO::new(
        "source-uuid".to_string(),
        "target-uuid".to_string(),
        "relation-name".to_string(),
        0.5,
    );

    let original_memory = MemoryDTO::new(node_dto, vec![], vec![edge_dto]);
    let serialized = serde_json::to_string(&original_memory).unwrap();
    let deserialized: MemoryDTO = serde_json::from_str(&serialized).unwrap();

    assert_eq!(original_memory.node.id, deserialized.node.id);
    assert_eq!(
        original_memory.outgoing_edges.len(),
        deserialized.outgoing_edges.len()
    );
    assert_eq!(
        original_memory.outgoing_edges[0].relation,
        deserialized.outgoing_edges[0].relation
    );
}

#[test]
fn test_custom_node_type_serialization() {
    let custom_type = NodeType::Custom("custom_type".to_string());
    let serialized = serde_json::to_string(&custom_type).unwrap();
    assert_eq!(serialized, "\"custom_type\"");

    let deserialized: NodeType = serde_json::from_str(&serialized).unwrap();
    assert_eq!(custom_type, deserialized);
}

#[test]
fn test_conversation_new_empty() {
    let conv = Conversation::new_empty();
    // Verify message list is empty
    assert!(conv.messages.is_empty());
    // Verify metadata map is empty
    assert!(conv.metadata.is_empty());
    // Verify unique non-zero ConversationId was generated
    assert_ne!(conv.id, ConversationId(ulid::Ulid::nil()));
}
