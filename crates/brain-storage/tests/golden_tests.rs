use brain_domain::{Edge, Node, NodeKind, RelationKind};
use std::fs;
use std::path::Path;

#[test]
fn test_golden_node_v1_compatibility() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let file_path = Path::new(&manifest_dir)
        .join("tests")
        .join("golden")
        .join("node_v1.json");
    let raw = fs::read_to_string(&file_path).unwrap();

    let node: Node = serde_json::from_str(&raw).unwrap();

    // Verify correct fields mapped
    assert_eq!(node.label, "SQLite");
    assert_eq!(node.node_type, NodeKind::Database);
    assert_eq!(
        node.properties.get("speed").unwrap().as_str().unwrap(),
        "fast"
    );

    // Verify reserialization is deterministic
    let serialized = serde_json::to_string_pretty(&node).unwrap();
    let clean_raw: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let clean_serialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(clean_raw, clean_serialized);
}

#[test]
fn test_golden_edge_v1_compatibility() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let file_path = Path::new(&manifest_dir)
        .join("tests")
        .join("golden")
        .join("edge_v1.json");
    let raw = fs::read_to_string(&file_path).unwrap();

    let edge: Edge = serde_json::from_str(&raw).unwrap();

    // Verify correct fields mapped
    assert_eq!(edge.relation, RelationKind::Uses);

    // Verify reserialization is deterministic
    let serialized = serde_json::to_string_pretty(&edge).unwrap();
    let clean_raw: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let clean_serialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(clean_raw, clean_serialized);
}

#[test]
fn test_golden_node_unknown_fallback() {
    let raw_unknown = r#"{
        "id": "c862d3a3-d022-4467-8ccb-7e6df5b61b47",
        "label": "Legacy Node",
        "node_type": "legacy_custom_class",
        "properties": {},
        "provenance": {
            "source_conversation": null,
            "source_message": null,
            "extracted_at": 1000,
            "extractor_version": "v1.0.0",
            "confidence": 1.0,
            "text_span": null
        },
        "updated_at": 1000
    }"#;

    let node: Node = serde_json::from_str(raw_unknown).unwrap();
    assert_eq!(node.node_type, NodeKind::Unknown);
}
