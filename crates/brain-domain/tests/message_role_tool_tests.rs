//! Inc 8: the Tool message role persists agentic-loop tool outcomes into
//! session transcripts.
use brain_domain::MessageRole;
use std::str::FromStr;

#[test]
fn tool_variant_displays_as_lowercase_tool() {
    assert_eq!(MessageRole::Tool.to_string(), "tool");
}

#[test]
fn tool_variant_serializes_and_deserializes_as_tool() {
    let json = serde_json::to_string(&MessageRole::Tool).unwrap();
    assert_eq!(json, r#""tool""#);
    let back: MessageRole = serde_json::from_str(&json).unwrap();
    assert_eq!(back, MessageRole::Tool);
}

#[test]
fn tool_variant_parses_from_str() {
    assert_eq!(MessageRole::from_str("tool").unwrap(), MessageRole::Tool);
}
