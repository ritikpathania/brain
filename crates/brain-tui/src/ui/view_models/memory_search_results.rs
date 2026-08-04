//! Presentation view models for knowledge graph search results.
//!
//! ## Architectural Invariant
//!
//! **ViewModels in this module are immutable presentation projections.**
//!
//! Their only responsibilities are:
//! - Formatting display strings
//! - Resolving `None` placeholder values at the presentation boundary
//! - Computing derived display values (badges, highlight ranges, confidence labels)
//! - Carrying presentation metadata (source kind, confidence tier)
//!
//! They must NOT contain:
//! - Retrieval logic or ranking algorithms
//! - Transport state or network error handling
//! - Mutable UI state or selection state
//! - Grouping or deduplication logic
//!
//! Grouping lives in [`MemoryGroupingEngine`], which is a deterministic
//! pure-function transform — not a ViewModel method.
//!
//! ## Pipeline
//!
//! ```text
//! SearchResult (retrieval boundary)
//!         ↓
//! MemoryResultViewModel::from_search_result (projection — this module)
//!         ↓
//! MemoryGroupingEngine::group (grouping — this module, not a ViewModel)
//!         ↓
//! MemoryResultGroup (presentation grouping container)
//!         ↓
//! Renderer
//! ```

use crate::client::Confidence;
use crate::ui::search::types::{SearchResult, SearchResultKind};

// ─── Detail Availability ────────────────────────────────────────────────────

/// Encodes whether a memory result has an expandable detail view available.
///
/// Replaces a plain `bool expandable` field, which can drift out of sync with
/// `entity_id`. Encoding the capability in the type makes invalid state
/// unrepresentable: if detail is `Available`, the `EntityId` to open it is
/// always present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailAvailability {
    /// No detail view is available for this result.
    None,
    /// A detail view can be opened using the given entity ID.
    Available(String),
}

impl DetailAvailability {
    /// Returns `true` if a detail view can be opened.
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available(_))
    }

    /// Returns the entity ID if detail is available.
    pub fn entity_id(&self) -> Option<&str> {
        match self {
            Self::Available(id) => Some(id.as_str()),
            Self::None => None,
        }
    }
}

// ─── MemoryResultViewModel ───────────────────────────────────────────────────

/// Immutable presentation projection of a single knowledge graph search result.
///
/// ## Projection rules
///
/// | `SearchResult` field | ViewModel field         | Resolution                          |
/// |----------------------|-------------------------|-------------------------------------|
/// | `title: None`        | `display_title`         | `"(untitled memory)"`               |
/// | `subtitle: None`     | `display_subtitle`      | `""`                                |
/// | `confidence`         | `confidence_badge`      | `"● HIGH"` / `"◐ MED"` / `"○ LOW"` |
/// | `kind == Knowledge`  | `detail`                | `DetailAvailability::Available(id)` |
/// | `kind != Knowledge`  | `detail`                | `DetailAvailability::None`          |
///
/// All display strings are fully resolved here — the renderer only reads,
/// it never branches on `Option`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryResultViewModel {
    /// Resolved display title — never empty, never a raw entity ID.
    pub display_title: String,
    /// Resolved subtitle or description — empty string if none.
    pub display_subtitle: String,
    /// Short confidence badge for inline display.
    /// Example: `"● HIGH"`, `"◐ MED "`, `"○ LOW "`.
    pub confidence_badge: String,
    /// Confidence tier for theme-driven colour selection.
    pub confidence: Confidence,
    /// Source system label (e.g. `"STM/LTM Graph"`).
    pub source_kind: String,
    /// Whether a detail drill-down view is available, and which entity to open.
    pub detail: DetailAvailability,
}

impl MemoryResultViewModel {
    /// Projects a [`SearchResult`] into a `MemoryResultViewModel`.
    ///
    /// This is the single authorised projection boundary for knowledge search
    /// results. All `Option` fields are resolved to display values here.
    ///
    /// Only `SearchResultKind::Knowledge` results carry `DetailAvailability::Available`.
    /// Other result kinds (Command, Session, Message) produce `DetailAvailability::None`.
    pub fn from_search_result(result: &SearchResult) -> Self {
        let display_title = result
            .title
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or("(untitled memory)")
            .to_string();

        let display_subtitle = result
            .subtitle
            .as_deref()
            .unwrap_or("")
            .to_string();

        let confidence = result.confidence;
        let confidence_badge = confidence_badge(confidence);

        // Only Knowledge results have a knowledge-graph entity that can be
        // drilled into. Commands, Sessions, and Messages have no entity detail.
        let detail = if result.kind == SearchResultKind::Knowledge && !result.entity_id.is_empty() {
            DetailAvailability::Available(result.entity_id.clone())
        } else {
            DetailAvailability::None
        };

        Self {
            display_title,
            display_subtitle,
            confidence_badge,
            confidence,
            source_kind: String::new(), // populated by group label; not on SearchResult
            detail,
        }
    }

    /// Returns `true` if this result has a drillable detail view.
    pub fn is_expandable(&self) -> bool {
        self.detail.is_available()
    }
}

/// Computes the short confidence badge string for inline rendering.
///
/// Uses Unicode block characters to give a visual weight signal without
/// depending on colour alone (WCAG 1.4.1 / non-colour cue).
///
/// - `●` (U+25CF) — filled circle for High
/// - `◐` (U+25D0) — half-filled for Medium
/// - `○` (U+25CB) — empty for Low
fn confidence_badge(c: Confidence) -> String {
    match c {
        Confidence::High   => "● HIGH".to_string(),
        Confidence::Medium => "◐ MED ".to_string(),
        Confidence::Low    => "○ LOW ".to_string(),
    }
}

// ─── MemoryResultGroup ───────────────────────────────────────────────────────

/// A labelled group of [`MemoryResultViewModel`] items for grouped presentation.
///
/// Groups are heuristic, deterministic, and stable across repeated calls on the
/// same input. They are pure presentation containers — they do NOT re-rank or
/// re-filter their members. The ordering within a group reflects the order of
/// the input `SearchResult` slice.
///
/// ## Group semantics
///
/// Groups are only a rendering hint. They have no effect on selection logic,
/// action dispatch, or retrieval. The renderer is responsible for deciding
/// whether to render group labels at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryResultGroup {
    /// Human-readable group label (e.g. `"High confidence"`, `"Recent sessions"`).
    pub label: String,
    /// Ordered items in this group.
    pub items: Vec<MemoryResultViewModel>,
}

impl MemoryResultGroup {
    /// Returns `true` if the group has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ─── MemoryGroupingEngine ────────────────────────────────────────────────────

/// Deterministic, stable grouping engine for knowledge search results.
///
/// ## Responsibilities
///
/// The engine assigns [`MemoryResultViewModel`] items into named
/// [`MemoryResultGroup`]s. Its single responsibility is grouping.
///
/// It must NOT:
/// - Re-rank items (order is fully determined by the upstream [`RankingEngine`])
/// - Filter items (filtering is fully determined by [`RankingConfig`])
/// - Perform retrieval or network calls
/// - Hold mutable state
///
/// ## Grouping strategy
///
/// Groups are assigned by confidence tier:
///
/// | Confidence | Group label        |
/// |------------|--------------------|
/// | High       | `"High confidence"` |
/// | Medium     | `"Good match"`     |
/// | Low        | `"Partial match"`  |
///
/// Empty groups are always omitted from the result. The group order is fixed:
/// High → Medium → Low, matching descending relevance.
///
/// This strategy is **heuristic** — it classifies by the confidence tier that
/// the retrieval pipeline assigned. The grouping does not perform any additional
/// retrieval reasoning.
///
/// [`RankingEngine`]: crate::ui::search::ranking::RankingEngine
/// [`RankingConfig`]: crate::ui::search::ranking::RankingConfig
pub struct MemoryGroupingEngine;

impl MemoryGroupingEngine {
    /// Groups `results` into labelled [`MemoryResultGroup`]s.
    ///
    /// The input ordering is preserved within each group.
    /// Empty groups are omitted.
    ///
    /// This function is a pure transform — calling it twice on the same input
    /// always produces the same output.
    pub fn group(results: &[SearchResult]) -> Vec<MemoryResultGroup> {
        let mut high: Vec<MemoryResultViewModel> = Vec::new();
        let mut medium: Vec<MemoryResultViewModel> = Vec::new();
        let mut low: Vec<MemoryResultViewModel> = Vec::new();

        for result in results {
            let vm = MemoryResultViewModel::from_search_result(result);
            match result.confidence {
                Confidence::High   => high.push(vm),
                Confidence::Medium => medium.push(vm),
                Confidence::Low    => low.push(vm),
            }
        }

        // Fixed group order: High → Medium → Low.
        // Empty groups are omitted.
        [
            (Confidence::High,   "High confidence", high),
            (Confidence::Medium, "Good match",      medium),
            (Confidence::Low,    "Partial match",   low),
        ]
        .into_iter()
        .filter_map(|(_, label, items)| {
            if items.is_empty() {
                None
            } else {
                Some(MemoryResultGroup {
                    label: label.to_string(),
                    items,
                })
            }
        })
        .collect()
    }

    /// Returns the total number of items across all groups.
    pub fn total_items(groups: &[MemoryResultGroup]) -> usize {
        groups.iter().map(|g| g.items.len()).sum()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::search::types::{SearchResultAction, SearchResultKind};
    use crate::ui::command::CommandId;

    fn make_result(
        entity_id: &str,
        title: Option<&str>,
        subtitle: Option<&str>,
        kind: SearchResultKind,
        confidence: Confidence,
    ) -> SearchResult {
        SearchResult {
            entity_id: entity_id.to_string(),
            title: title.map(str::to_string),
            subtitle: subtitle.map(str::to_string),
            kind,
            provider_score: 50,
            confidence,
            action: SearchResultAction::InvokeCommand(CommandId("dummy")),
        }
    }

    // ── DetailAvailability ──────────────────────────────────────────────────

    #[test]
    fn test_detail_availability_available_carries_entity_id() {
        let d = DetailAvailability::Available("ent-123".to_string());
        assert!(d.is_available());
        assert_eq!(d.entity_id(), Some("ent-123"));
    }

    #[test]
    fn test_detail_availability_none_has_no_entity_id() {
        let d = DetailAvailability::None;
        assert!(!d.is_available());
        assert_eq!(d.entity_id(), None);
    }

    // ── MemoryResultViewModel projection ───────────────────────────────────

    #[test]
    fn test_knowledge_result_with_title_projects_correctly() {
        let result = make_result(
            "ent-abc",
            Some("Rust Ownership"),
            Some("Memory safety without GC"),
            SearchResultKind::Knowledge,
            Confidence::High,
        );
        let vm = MemoryResultViewModel::from_search_result(&result);

        assert_eq!(vm.display_title, "Rust Ownership");
        assert_eq!(vm.display_subtitle, "Memory safety without GC");
        assert_eq!(vm.confidence_badge, "● HIGH");
        assert_eq!(vm.confidence, Confidence::High);
        assert!(vm.is_expandable());
        assert_eq!(vm.detail, DetailAvailability::Available("ent-abc".to_string()));
    }

    #[test]
    fn test_none_title_resolves_to_placeholder_at_viewmodel_boundary() {
        // Invariant: None title is never passed to the renderer.
        // It must be resolved here, not in the widget.
        let result = make_result(
            "ent-xyz",
            None,
            None,
            SearchResultKind::Knowledge,
            Confidence::Medium,
        );
        let vm = MemoryResultViewModel::from_search_result(&result);

        assert_eq!(vm.display_title, "(untitled memory)");
        assert_eq!(vm.display_subtitle, "");
    }

    #[test]
    fn test_empty_title_resolves_to_placeholder() {
        let result = make_result(
            "ent-empty",
            Some("  "), // whitespace-only
            None,
            SearchResultKind::Knowledge,
            Confidence::Low,
        );
        let vm = MemoryResultViewModel::from_search_result(&result);
        assert_eq!(vm.display_title, "(untitled memory)");
    }

    #[test]
    fn test_non_knowledge_result_has_no_detail_availability() {
        // Commands and sessions have no knowledge-graph entity to drill into.
        let session_result = make_result(
            "session-123",
            Some("Previous session"),
            None,
            SearchResultKind::Session,
            Confidence::High,
        );
        let vm = MemoryResultViewModel::from_search_result(&session_result);
        assert_eq!(vm.detail, DetailAvailability::None);
        assert!(!vm.is_expandable());
    }

    #[test]
    fn test_knowledge_result_with_empty_entity_id_has_no_detail() {
        // A Knowledge result with no entity_id cannot be opened — entity_id is empty.
        let result = make_result(
            "", // empty entity_id
            Some("Some memory"),
            None,
            SearchResultKind::Knowledge,
            Confidence::Medium,
        );
        let vm = MemoryResultViewModel::from_search_result(&result);
        assert_eq!(vm.detail, DetailAvailability::None);
    }

    #[test]
    fn test_confidence_badge_strings() {
        assert_eq!(confidence_badge(Confidence::High),   "● HIGH");
        assert_eq!(confidence_badge(Confidence::Medium), "◐ MED ");
        assert_eq!(confidence_badge(Confidence::Low),    "○ LOW ");
    }

    // ── MemoryGroupingEngine ────────────────────────────────────────────────

    #[test]
    fn test_grouping_produces_fixed_order_high_medium_low() {
        let results = vec![
            make_result("a", Some("Low result"),    None, SearchResultKind::Knowledge, Confidence::Low),
            make_result("b", Some("High result"),   None, SearchResultKind::Knowledge, Confidence::High),
            make_result("c", Some("Medium result"), None, SearchResultKind::Knowledge, Confidence::Medium),
        ];

        let groups = MemoryGroupingEngine::group(&results);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].label, "High confidence");
        assert_eq!(groups[1].label, "Good match");
        assert_eq!(groups[2].label, "Partial match");

        assert_eq!(groups[0].items[0].display_title, "High result");
        assert_eq!(groups[1].items[0].display_title, "Medium result");
        assert_eq!(groups[2].items[0].display_title, "Low result");
    }

    #[test]
    fn test_empty_groups_are_omitted() {
        let results = vec![
            make_result("a", Some("High A"), None, SearchResultKind::Knowledge, Confidence::High),
            make_result("b", Some("High B"), None, SearchResultKind::Knowledge, Confidence::High),
        ];

        let groups = MemoryGroupingEngine::group(&results);
        assert_eq!(groups.len(), 1, "Only High group should exist");
        assert_eq!(groups[0].label, "High confidence");
        assert_eq!(groups[0].items.len(), 2);
    }

    #[test]
    fn test_empty_input_produces_no_groups() {
        let groups = MemoryGroupingEngine::group(&[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_grouping_preserves_input_order_within_group() {
        // Items within a group must appear in input order (ranking is upstream).
        let results = vec![
            make_result("first",  Some("Alpha"), None, SearchResultKind::Knowledge, Confidence::High),
            make_result("second", Some("Beta"),  None, SearchResultKind::Knowledge, Confidence::High),
            make_result("third",  Some("Gamma"), None, SearchResultKind::Knowledge, Confidence::High),
        ];

        let groups = MemoryGroupingEngine::group(&results);
        assert_eq!(groups.len(), 1);
        let items = &groups[0].items;
        assert_eq!(items[0].display_title, "Alpha");
        assert_eq!(items[1].display_title, "Beta");
        assert_eq!(items[2].display_title, "Gamma");
    }

    #[test]
    fn test_grouping_is_deterministic() {
        let results = vec![
            make_result("a", Some("High"),   None, SearchResultKind::Knowledge, Confidence::High),
            make_result("b", Some("Medium"), None, SearchResultKind::Knowledge, Confidence::Medium),
            make_result("c", Some("Low"),    None, SearchResultKind::Knowledge, Confidence::Low),
        ];

        let first  = MemoryGroupingEngine::group(&results);
        let second = MemoryGroupingEngine::group(&results);
        assert_eq!(first, second, "Grouping must be deterministic");
    }

    #[test]
    fn test_grouping_does_not_rerank_within_group() {
        // The engine must NOT re-sort within a group. Input order must be preserved.
        let results = vec![
            // Deliberately non-alphabetical within the High tier
            make_result("z", Some("Zebra"), None, SearchResultKind::Knowledge, Confidence::High),
            make_result("a", Some("Apple"), None, SearchResultKind::Knowledge, Confidence::High),
        ];

        let groups = MemoryGroupingEngine::group(&results);
        assert_eq!(groups[0].items[0].display_title, "Zebra");
        assert_eq!(groups[0].items[1].display_title, "Apple");
    }

    #[test]
    fn test_total_items_matches_input_count() {
        let results = vec![
            make_result("a", Some("H"), None, SearchResultKind::Knowledge, Confidence::High),
            make_result("b", Some("M"), None, SearchResultKind::Knowledge, Confidence::Medium),
            make_result("c", Some("L"), None, SearchResultKind::Knowledge, Confidence::Low),
        ];

        let groups = MemoryGroupingEngine::group(&results);
        assert_eq!(MemoryGroupingEngine::total_items(&groups), 3);
    }

    #[test]
    fn test_grouping_skips_non_knowledge_results_detail_availability() {
        // Grouping works on any SearchResult kind — the detail field is set by the ViewModel.
        let results = vec![
            make_result("cmd", Some("A command"), None, SearchResultKind::Command, Confidence::High),
            make_result("ent", Some("A memory"),  None, SearchResultKind::Knowledge, Confidence::High),
        ];

        let groups = MemoryGroupingEngine::group(&results);
        assert_eq!(groups.len(), 1);
        let items = &groups[0].items;

        // Command: no detail
        assert_eq!(items[0].detail, DetailAvailability::None);
        // Knowledge: detail available
        assert_eq!(items[1].detail, DetailAvailability::Available("ent".to_string()));
    }
}
