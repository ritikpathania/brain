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

    // Assert Clone trait for EdgeId (non-Copy due to relation identifier)
    let edge_id1 = EdgeId::new(node_id1, NodeId::new(), RelationId::new("knows"));
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

    let edge_id = EdgeId::new(node_id, NodeId::new(), RelationId::new("related_to"));
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
    let original_edge = Edge::new(source_id, target_id, RelationKind::AssociatedWith, 0.85);

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

    // 4. Session Roundtrip
    let session_id = SessionId::new();
    let msg1 = Message::new(MessageId::new(), MessageRole::User, "hello".to_string());
    let msg2 = Message::new(
        MessageId::new(),
        MessageRole::Assistant,
        "hi there".to_string(),
    );

    let original_session = Session::reconstruct(
        session_id,
        SessionTitle("test title".to_string()),
        false,
        true,
        vec![msg1, msg2],
        vec![Goal { id: GoalId::new(), text: "test goal".to_string() }],
        SessionTimestamp(123456),
    );

    let serialized_session = serde_json::to_string(&original_session).unwrap();
    let deserialized_session: Session = serde_json::from_str(&serialized_session).unwrap();

    assert_eq!(original_session.id, deserialized_session.id);
    assert_eq!(
        original_session.messages.len(),
        deserialized_session.messages.len()
    );
    assert_eq!(
        original_session.messages[0].content,
        deserialized_session.messages[0].content
    );
    assert_eq!(original_session.title, deserialized_session.title);
    assert_eq!(original_session.pinned, deserialized_session.pinned);
    assert_eq!(original_session.archived, deserialized_session.archived);
    assert_eq!(original_session.goals.len(), deserialized_session.goals.len());
    assert_eq!(original_session.goals[0].text, deserialized_session.goals[0].text);
    assert_eq!(original_session.updated_at, deserialized_session.updated_at);
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
    let serialized = "\"custom_type\"";
    let deserialized: NodeType = serde_json::from_str(serialized).unwrap();
    assert_eq!(NodeType::Unknown, deserialized);
}

#[test]
fn test_session_new() {
    let id = SessionId::new();
    let title = SessionTitle("New Session".to_string());
    let timestamp = SessionTimestamp(0);
    let session = Session::new(id, title.clone(), timestamp);
    // Verify message list is empty
    assert!(session.messages.is_empty());
    // Verify default fields
    assert_eq!(session.title, title);
    assert!(!session.archived);
    assert!(!session.pinned);
    assert!(session.goals.is_empty());
}

#[test]
fn test_bkf_document_roundtrip() {
    use brain_domain::bkf::*;
    use std::collections::HashMap;

    let mut builder = BKFDocumentBuilder::new(
        BkfDocumentId::new(),
        "bkf".to_string(),
        "1.0.0".to_string(),
    ).with_metadata(Metadata {
        title: Some("Integration Spec".to_string()),
        author: Some("Agent".to_string()),
        checksum: Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()),
        fingerprint: Some("fingerprint-123".to_string()),
        language: Some("en".to_string()),
        mime: Some("text/markdown".to_string()),
        size: Some(1024),
        created: Some(1700000000),
        modified: Some(1700001000),
        license: Some("MIT".to_string()),
        extra: HashMap::new(),
    });

    let block_id = BkfBlockId::new();
    builder.add_block(Block {
        id: block_id,
        content: BlockContent::Paragraph("Hello BKF".to_string()),
        provenance: vec![Provenance::MarkdownFile {
            path: "README.md".to_string(),
            repository: None,
            commit: None,
            author: None,
            timestamp: 1700000000,
        }],
        tags: vec!["intro".to_string()],
    });

    let entity_id = BkfEntityId::new();
    builder.add_entity(Entity {
        id: entity_id,
        entity_type: "Concept".to_string(),
        name: "BKF".to_string(),
        aliases: vec![],
        attributes: HashMap::new(),
        confidence: 0.95,
    });

    builder.add_relationship(Relationship {
        id: BkfRelationshipId::new(),
        source: BKFTargetRef::Block(block_id),
        target: BKFTargetRef::Entity(entity_id),
        relationship_type: RelationshipType::REFERENCES,
        confidence: 0.9,
        provenance: vec![],
    });

    let doc = builder.build().unwrap();

    let serialized = serde_json::to_string(&doc).unwrap();
    let deserialized: BKFDocument = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(doc.id(), deserialized.id());
    assert_eq!(deserialized.metadata().title.as_deref(), Some("Integration Spec"));
    assert!(deserialized.capabilities().has_blocks);
    assert!(deserialized.capabilities().has_entities);
    assert!(deserialized.capabilities().has_relationships);
    assert!(!deserialized.capabilities().has_facts);
}

#[test]
fn test_bkf_builder_validations() {
    use brain_domain::bkf::*;
    use std::collections::HashMap;

    let meta = Metadata {
        title: Some("V".to_string()),
        author: None,
        checksum: None,
        fingerprint: None,
        language: None,
        mime: None,
        size: None,
        created: None,
        modified: None,
        license: None,
        extra: HashMap::new(),
    };

    // 1. Referential integrity validation failure (missing source/target)
    let mut builder = BKFDocumentBuilder::new(
        BkfDocumentId::new(),
        "bkf".to_string(),
        "1.0.0".to_string(),
    ).with_metadata(meta.clone());

    builder.add_relationship(Relationship {
        id: BkfRelationshipId::new(),
        source: BKFTargetRef::Entity(BkfEntityId::new()),
        target: BKFTargetRef::Block(BkfBlockId::new()),
        relationship_type: RelationshipType::CALLS,
        confidence: 0.8,
        provenance: vec![],
    });
    assert!(matches!(builder.build(), Err(BkfError::MissingReference { .. })));

    // 2. Duplicate ID validation failure
    let dup_id = BkfBlockId::new();
    let mut builder2 = BKFDocumentBuilder::new(
        BkfDocumentId::new(),
        "bkf".to_string(),
        "1.0.0".to_string(),
    ).with_metadata(meta.clone());

    builder2.add_block(Block {
        id: dup_id,
        content: BlockContent::Paragraph("A".to_string()),
        provenance: vec![],
        tags: vec![],
    });
    builder2.add_block(Block {
        id: dup_id,
        content: BlockContent::Paragraph("B".to_string()),
        provenance: vec![],
        tags: vec![],
    });
    assert!(matches!(builder2.build(), Err(BkfError::DuplicateId(..))));

    // 3. Cyclic section hierarchy failure
    let sec1_id = BkfSectionId::new();
    let sec2_id = BkfSectionId::new();
    let mut builder3 = BKFDocumentBuilder::new(
        BkfDocumentId::new(),
        "bkf".to_string(),
        "1.0.0".to_string(),
    ).with_metadata(meta.clone());

    builder3.add_section(Section {
        id: sec1_id,
        title: "S1".to_string(),
        level: 1,
        parent_id: Some(sec2_id),
        block_ids: vec![],
    });
    builder3.add_section(Section {
        id: sec2_id,
        title: "S2".to_string(),
        level: 2,
        parent_id: Some(sec1_id),
        block_ids: vec![],
    });
    assert!(matches!(builder3.build(), Err(BkfError::CycleDetected { .. })));

    // 4. Self-referencing relationship validation failure
    let ent_id = BkfEntityId::new();
    let mut builder4 = BKFDocumentBuilder::new(
        BkfDocumentId::new(),
        "bkf".to_string(),
        "1.0.0".to_string(),
    ).with_metadata(meta);

    builder4.add_entity(Entity {
        id: ent_id,
        entity_type: "concept".to_string(),
        name: "E1".to_string(),
        aliases: vec![],
        attributes: HashMap::new(),
        confidence: 0.9,
    });
    builder4.add_relationship(Relationship {
        id: BkfRelationshipId::new(),
        source: BKFTargetRef::Entity(ent_id),
        target: BKFTargetRef::Entity(ent_id),
        relationship_type: RelationshipType::DEPENDS_ON,
        confidence: 0.9,
        provenance: vec![],
    });
    assert!(matches!(builder4.build(), Err(BkfError::SelfReferencing { .. })));
}

#[test]
fn test_bkf_schema_evolution() {
    // JSON representation of an older schema version (v0.1.0)
    let old_json = r#"{
        "id": "01H2N2Z8D992Y1Z75V0C2RQXWP",
        "schema_name": "bkf",
        "schema_version": "0.1.0",
        "capabilities": {
            "has_sections": false,
            "has_blocks": false,
            "has_entities": false,
            "has_relationships": false,
            "has_facts": false,
            "has_citations": false,
            "has_attachments": false,
            "has_embeddings": false
        },
        "metadata": {
            "title": "Old Doc",
            "author": null,
            "checksum": null,
            "fingerprint": null,
            "language": null,
            "mime": null,
            "size": null,
            "created": null,
            "modified": null,
            "license": null,
            "extra": {}
        },
        "sections": [],
        "blocks": [],
        "entities": [],
        "relationships": [],
        "facts": [],
        "citations": [],
        "attachments": [],
        "provenance": [],
        "embeddings": [],
        "chunk_refs": [],
        "tags": [],
        "custom_metadata": {}
    }"#;

    let doc: brain_domain::bkf::BKFDocument = serde_json::from_str(old_json).unwrap();
    assert_eq!(doc.schema_version(), "0.1.0");
    assert_eq!(doc.metadata().title.as_deref(), Some("Old Doc"));
}

