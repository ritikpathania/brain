//! Inc 19: the Thinking message role persists reasoning blocks into
//! session transcripts.
use brain_domain::MessageRole;
use std::str::FromStr;

#[test]
fn thinking_variant_displays_as_lowercase_thinking() {
    assert_eq!(MessageRole::Thinking.to_string(), "thinking");
}

#[test]
fn thinking_variant_serializes_and_deserializes_as_thinking() {
    let json = serde_json::to_string(&MessageRole::Thinking).unwrap();
    assert_eq!(json, r#""thinking""#);
    let back: MessageRole = serde_json::from_str(&json).unwrap();
    assert_eq!(back, MessageRole::Thinking);
}

#[test]
fn thinking_variant_parses_from_str() {
    assert_eq!(MessageRole::from_str("thinking").unwrap(), MessageRole::Thinking);
}
