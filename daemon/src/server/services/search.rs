//! Pure domain SearchService executing hybrid projection searches over BrainRuntime.

use crate::server::protocol::{SearchPayload, SearchResponsePayload, SearchResultItem};
use brain_core::events::CorrelationId;
use brain_services::BrainRuntime;
use std::borrow::Cow;
use std::sync::Arc;

/// Constructs the best available human-readable display label for a graph node.
///
/// Priority order:
/// 1. `label` — if it is not a UUID, not a JSON fragment, and not empty,
///    return it as-is (`Cow::Borrowed`, zero allocation).
/// 2. `node_type` — if the label is unusable, derive a descriptive fallback
///    from the node's semantic type (e.g. `"session_context"` → `"Conversation"`).
/// 3. `"Untitled memory"` — last resort when neither source yields useful text.
///
/// Returning uniform `"Memory fragment"` for every bad label is avoided because
/// it produces indistinguishable rows in multi-result views (50× "Memory fragment").
///
/// # Follow-up
/// If `brain-domain`'s `NodeType` becomes an enum accessible here, prefer:
/// `fn build_display_label<'a>(label: &'a str, kind: NodeKind) -> Cow<'a, str>`
/// for compile-time exhaustiveness and zero `to_lowercase()` allocation.
pub(crate) fn build_display_label<'a>(label: &'a str, node_type: &str) -> Cow<'a, str> {
    let trimmed = label.trim();

    // Fast path: label is already valid — borrow without allocating.
    if !trimmed.is_empty()
        && !trimmed.starts_with('{')
        && !trimmed.starts_with('[')
        && !is_uuid(trimmed)
    {
        return Cow::Borrowed(label);
    }

    // Slow path: label is unusable — derive from node_type.
    let type_fallback = match node_type.to_lowercase().as_str() {
        t if t.contains("session") || t.contains("conversation") => "Conversation",
        t if t.contains("concept") || t.contains("knowledge")    => "Knowledge",
        t if t.contains("observation")                           => "Observation",
        t if t.contains("document") || t.contains("note")        => "Document",
        t if t.contains("entity") || t.contains("person")        => "Entity",
        _                                                         => "Untitled memory",
    };

    Cow::Owned(type_fallback.to_string())
}

/// Returns true if `s` matches the UUID hex-hyphen pattern (8-4-4-4-12).
fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    matches!(parts.as_slice(), [a, b, c, d, e]
        if a.len() == 8 && b.len() == 4 && c.len() == 4 && d.len() == 4 && e.len() == 12
        && [a, b, c, d, e].iter().all(|p| p.chars().all(|ch| ch.is_ascii_hexdigit()))
    )
}

/// Domain service executing graph and vector search queries across BrainRuntime.
pub struct SearchService;

impl SearchService {
    /// Executes a search query and returns structured domain SearchResultItems.
    pub fn execute(
        payload: &SearchPayload,
        brain_runtime: &Arc<BrainRuntime>,
    ) -> Result<SearchResponsePayload, String> {
        let limit = payload.limit.unwrap_or(20);
        let rt_corr_id = CorrelationId::new_v4();

        let search_query = brain_services::SearchProjectionQuery {
            query: payload.query.clone(),
            limit,
        };
        let search_projector = brain_services::SearchProjector;

        let runtime_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            brain_runtime.query_projection(&search_projector, &search_query, rt_corr_id)
        }));

        match runtime_res {
            Ok(projection_result) => {
                let mut items = Vec::new();
                for (node, score) in projection_result.items {
                    // Build the best available display label — never raw UUID or JSON.
                    let display_label = build_display_label(
                        &node.label,
                        &node.node_type.to_string(),
                    );
                    items.push(SearchResultItem {
                        id: node.id.to_string(),
                        label: display_label.as_ref().to_string(),
                        score,
                        source_kind: "STM/LTM Graph".to_string(),
                        content: display_label.into_owned(),
                    });
                }

                // Workspace context boosting
                let ws_set: std::collections::HashSet<String> =
                    payload.workspace_context.iter().cloned().collect();

                if !ws_set.is_empty() {
                    let (mut ws_items, other_items): (Vec<_>, Vec<_>) =
                        items.into_iter().partition(|item| ws_set.contains(&item.id));
                    ws_items.extend(other_items);
                    items = ws_items;
                }

                let context_used: Vec<String> = items
                    .iter()
                    .filter(|item| ws_set.contains(&item.id))
                    .map(|item| item.id.clone())
                    .collect();

                Ok(SearchResponsePayload { items, context_used })
            }
            Err(_) => Err("BrainRuntime search projection panicked".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_label_is_borrowed_unchanged() {
        let label = "Introduction to Rust";
        let result = build_display_label(label, "concept");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "Introduction to Rust");
    }

    #[test]
    fn test_uuid_label_falls_back_to_node_type_name() {
        let uuid = "7e052392-b328-4eb5-9a1d-deadbeef1234";
        let result = build_display_label(uuid, "session_context");
        assert_eq!(result.as_ref(), "Conversation");
    }

    #[test]
    fn test_session_type_label_becomes_conversation() {
        let uuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        assert_eq!(build_display_label(uuid, "session").as_ref(), "Conversation");
        assert_eq!(build_display_label(uuid, "conversation").as_ref(), "Conversation");
        assert_eq!(build_display_label(uuid, "session_context").as_ref(), "Conversation");
    }

    #[test]
    fn test_json_label_falls_back_to_node_type_name() {
        let json = r#"{"event":"text","content":"hello"}"#;
        let result = build_display_label(json, "observation");
        assert_eq!(result.as_ref(), "Observation");
    }

    #[test]
    fn test_unknown_type_uses_untitled_memory() {
        let uuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let result = build_display_label(uuid, "unknown_kind");
        assert_eq!(result.as_ref(), "Untitled memory");
    }

    #[test]
    fn test_fifty_uuid_results_have_distinct_fallbacks() {
        // Regression guard: 50 nodes of different types must produce ≥2 distinct labels.
        let uuids: Vec<String> = (0..50)
            .map(|i| format!("aaaaaaaa-bbbb-cccc-dddd-{:012x}", i))
            .collect();
        let types = ["session_context", "concept", "observation", "document", "entity"];
        let labels: std::collections::HashSet<String> = uuids
            .iter()
            .enumerate()
            .map(|(i, uuid)| build_display_label(uuid, types[i % types.len()]).into_owned())
            .collect();
        assert!(labels.len() >= 2, "All 50 results collapsed to uniform fallback");
    }

    #[test]
    fn test_empty_label_falls_back_to_node_type_name() {
        let result = build_display_label("", "concept");
        assert_eq!(result.as_ref(), "Knowledge");
    }
}


use crate::server::protocol::{SearchPayload, SearchResponsePayload, SearchResultItem};
use brain_core::events::CorrelationId;
use brain_services::BrainRuntime;
use std::sync::Arc;

/// Domain service executing graph and vector search queries across BrainRuntime.
pub struct SearchService;

impl SearchService {
    /// Executes a search query and returns structured domain SearchResultItems.
    pub fn execute(
        payload: &SearchPayload,
        brain_runtime: &Arc<BrainRuntime>,
    ) -> Result<SearchResponsePayload, String> {
        let limit = payload.limit.unwrap_or(20);
        let rt_corr_id = CorrelationId::new_v4();

        let search_query = brain_services::SearchProjectionQuery {
            query: payload.query.clone(),
            limit,
        };
        let search_projector = brain_services::SearchProjector;

        let runtime_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            brain_runtime.query_projection(&search_projector, &search_query, rt_corr_id)
        }));

        match runtime_res {
            Ok(projection_result) => {
                let mut items = Vec::new();
                for (node, score) in projection_result.items {
                    items.push(SearchResultItem {
                        id: node.id.to_string(),
                        label: node.label.clone(),
                        score,
                        source_kind: "STM/LTM Graph".to_string(),
                        content: node.label,
                    });
                }

                // Workspace context boosting
                let ws_set: std::collections::HashSet<String> =
                    payload.workspace_context.iter().cloned().collect();

                if !ws_set.is_empty() {
                    let (mut ws_items, other_items): (Vec<_>, Vec<_>) =
                        items.into_iter().partition(|item| ws_set.contains(&item.id));
                    ws_items.extend(other_items);
                    items = ws_items;
                }

                let context_used: Vec<String> = items
                    .iter()
                    .filter(|item| ws_set.contains(&item.id))
                    .map(|item| item.id.clone())
                    .collect();

                Ok(SearchResponsePayload { items, context_used })
            }
            Err(_) => Err("BrainRuntime search projection panicked".to_string()),
        }
    }
}
