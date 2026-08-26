//! Property recovery for memory/search: the application-layer projection
//! drops node properties (the `SearchMetadata` enum carries none), so
//! relations/excerpt/scope are read back from the stored node here.

use serde_json::{Map, Value};

/// Resolve relations from a stored node's property map, tolerating native
/// JSON arrays and legacy string-encoded JSON, falling back to the summary
/// metadata string when the node carries nothing usable. Non-array garbage
/// falls through; never errors.
pub fn extract_relations(
    props: Option<&Map<String, Value>>,
    metadata_fallback: Option<&str>,
) -> Vec<Value> {
    let decode = |raw: &Value| -> Option<Vec<Value>> {
        match raw {
            Value::Array(items) => Some(items.clone()),
            Value::String(s) => serde_json::from_str::<Vec<Value>>(s).ok(),
            _ => None,
        }
    };
    if let Some(map) = props {
        if let Some(raw) = map.get("relations") {
            if let Some(rels) = decode(raw) {
                return rels;
            }
        }
    }
    if let Some(encoded) = metadata_fallback {
        if let Ok(rels) = serde_json::from_str::<Vec<Value>>(encoded) {
            return rels;
        }
    }
    Vec::new()
}

/// Prefer the stored `content` property as the excerpt; fall back to whatever
/// body the retrieval pipeline produced (today: the node label).
pub fn preferred_excerpt(props: Option<&Map<String, Value>>, fallback_body: &str) -> String {
    props
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_body.to_string())
}

/// Resolve scope from properties, falling back to the provided default.
pub fn preferred_scope(props: Option<&Map<String, Value>>, fallback: &str) -> String {
    props
        .and_then(|m| m.get("scope"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn relations_from_native_array_property() {
        let props = map(json!({"relations": [{"relation": "supports", "target_id": "beta-1"}]}));
        let rels = extract_relations(Some(&props), None);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["target_id"], "beta-1");
    }

    #[test]
    fn relations_from_legacy_string_encoded_property() {
        let props = map(json!({"relations": "[{\"relation\":\"supports\"}]"}));
        assert_eq!(extract_relations(Some(&props), None).len(), 1);
    }

    #[test]
    fn relations_fall_back_to_summary_metadata_string() {
        let encoded = "[{\"relation\":\"supports\",\"target_id\":\"x\"}]";
        assert_eq!(extract_relations(None, Some(encoded)).len(), 1);
    }

    #[test]
    fn relations_empty_when_nothing_carries_them() {
        let props = map(json!({"content": "just prose"}));
        let rels = extract_relations(Some(&props), Some("not json"));
        assert!(rels.is_empty());
    }

    #[test]
    fn relations_garbage_property_falls_through_to_metadata() {
        let props = map(json!({"relations": 42}));
        let rels = extract_relations(Some(&props), Some("[]"));
        assert!(rels.is_empty());
    }

    #[test]
    fn empty_array_is_preserved_not_treated_as_missing() {
        // A node stored WITH an empty relations list must not silently
        // resurrect entries from the metadata fallback.
        let props = map(json!({"relations": []}));
        assert!(extract_relations(Some(&props), Some("[{\"relation\":\"x\"}]")).is_empty());
    }

    #[test]
    fn excerpt_prefers_content_over_label_body() {
        let props = map(json!({"content": "real prose body"}));
        assert_eq!(preferred_excerpt(Some(&props), "Alpha Label"), "real prose body");
    }

    #[test]
    fn excerpt_blank_content_falls_back() {
        let props = map(json!({"content": "   "}));
        assert_eq!(preferred_excerpt(Some(&props), "Alpha Label"), "Alpha Label");
    }

    #[test]
    fn excerpt_without_props_returns_fallback() {
        assert_eq!(preferred_excerpt(None, "Alpha Label"), "Alpha Label");
    }

    #[test]
    fn scope_prefers_property_then_default() {
        let props = map(json!({"scope": "compiler"}));
        assert_eq!(preferred_scope(Some(&props), "workspace"), "compiler");
        assert_eq!(preferred_scope(None, "workspace"), "workspace");
    }
}
