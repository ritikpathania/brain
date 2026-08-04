//! E2E Search Flow behavioral tests.
//!
//! Verifies concept search projection, confidence grouping, placeholder resolution,
//! and ordering stability across repaints.

use brain_tui::client::Confidence;
use brain_tui::ui::search::ranking::RankingConfig;
use brain_tui::ui::search::types::{SearchResult, SearchResultAction, SearchResultKind};
use brain_tui::ui::view_models::memory_search_results::{
    DetailAvailability, MemoryGroupingEngine, MemoryResultViewModel,
};

#[test]
fn test_concept_search_and_confidence_grouping() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    let results: Vec<SearchResult> = vec![
        SearchResult {
            entity_id: "ent-high-1".to_string(),
            title: Some("Domain Driven Design".to_string()),
            subtitle: Some("Core DDD patterns and aggregates".to_string()),
            kind: SearchResultKind::Knowledge,
            provider_score: 95,
            confidence: Confidence::High,
            action: SearchResultAction::OpenMemoryDetail {
                entity_id: "ent-high-1".to_string(),
            },
        },
        SearchResult {
            entity_id: "ent-med-1".to_string(),
            title: None, // Missing title -> should resolve to "(untitled memory)"
            subtitle: None, // Missing summary -> should resolve to ""
            kind: SearchResultKind::Knowledge,
            provider_score: 70,
            confidence: Confidence::Medium,
            action: SearchResultAction::OpenMemoryDetail {
                entity_id: "ent-med-1".to_string(),
            },
        },
        SearchResult {
            entity_id: "ent-low-1".to_string(),
            title: Some("Legacy Refactor".to_string()),
            subtitle: Some("Historical migration details".to_string()),
            kind: SearchResultKind::Knowledge,
            provider_score: 40,
            confidence: Confidence::Low,
            action: SearchResultAction::OpenMemoryDetail {
                entity_id: "ent-low-1".to_string(),
            },
        },
    ];

    // ── Act ──────────────────────────────────────────────────────────────────
    let groups = MemoryGroupingEngine::group(&results);

    // ── Assert (Black-Box Observable Behavior) ───────────────────────────────
    assert_eq!(groups.len(), 3, "Should create 3 groups for High, Medium, Low");
    assert_eq!(groups[0].label, "High confidence");
    assert_eq!(groups[1].label, "Good match");
    assert_eq!(groups[2].label, "Partial match");

    // Group 1 (High match)
    let high_vm = &groups[0].items[0];
    assert_eq!(high_vm.display_title, "Domain Driven Design");
    assert_eq!(high_vm.display_subtitle, "Core DDD patterns and aggregates");
    assert_eq!(
        high_vm.detail,
        DetailAvailability::Available("ent-high-1".to_string())
    );

    // Group 2 (Medium match — testing placeholder resolution)
    let med_vm = &groups[1].items[0];
    assert_eq!(
        med_vm.display_title, "(untitled memory)",
        "Missing title MUST resolve to placeholder '(untitled memory)'"
    );
    assert_eq!(med_vm.display_subtitle, "", "Missing summary MUST resolve to empty string");
    assert_eq!(
        med_vm.detail,
        DetailAvailability::Available("ent-med-1".to_string()),
        "Valid entity ID MUST be preserved in DetailAvailability::Available"
    );

    // Ensure raw UUID strings are never displayed as titles
    assert!(!med_vm.display_title.contains("ent-med-1"));
}

#[test]
fn test_presentation_placeholder_resolution_invariant() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    // Candidate with missing title, missing summary, valid entity ID
    let result = SearchResult {
        entity_id: "uuid-999-aaa".to_string(),
        title: None,
        subtitle: None,
        kind: SearchResultKind::Knowledge,
        provider_score: 85,
        confidence: Confidence::High,
        action: SearchResultAction::OpenMemoryDetail {
            entity_id: "uuid-999-aaa".to_string(),
        },
    };

    // ── Act ──────────────────────────────────────────────────────────────────
    let vm = MemoryResultViewModel::from_search_result(&result);

    // ── Assert (Black-Box Observable Boundary Invariants) ────────────────────
    assert_eq!(vm.display_title, "(untitled memory)");
    assert_eq!(vm.display_subtitle, "");
    assert_eq!(
        vm.detail,
        DetailAvailability::Available("uuid-999-aaa".to_string())
    );
}

#[test]
fn test_ordering_stability_across_repaints() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    let results: Vec<SearchResult> = vec![
        ("e1", Confidence::High, 90, "First High"),
        ("e2", Confidence::High, 80, "Second High"),
        ("e3", Confidence::Medium, 60, "First Med"),
        ("e4", Confidence::Low, 30, "First Low"),
    ]
    .into_iter()
    .map(|(id, conf, provider_score, title)| SearchResult {
        entity_id: id.to_string(),
        title: Some(title.to_string()),
        subtitle: None,
        kind: SearchResultKind::Knowledge,
        provider_score,
        confidence: conf,
        action: SearchResultAction::OpenMemoryDetail {
            entity_id: id.to_string(),
        },
    })
    .collect();

    // ── Act: Group multiple times (simulating repaints/redraws) ─────────────
    let run1 = MemoryGroupingEngine::group(&results);
    let run2 = MemoryGroupingEngine::group(&results);
    let run3 = MemoryGroupingEngine::group(&results);

    // ── Assert: Group and item order MUST be 100% identical ─────────────────
    assert_eq!(run1, run2, "Grouping must be 100% identical across repaints");
    assert_eq!(run2, run3, "Grouping must be 100% identical across repaints");

    // Item titles order within High group
    assert_eq!(run1[0].items[0].display_title, "First High");
    assert_eq!(run1[0].items[1].display_title, "Second High");
}

#[test]
fn test_empty_search_results_flow() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    let empty_results: Vec<SearchResult> = Vec::new();

    // ── Act ──────────────────────────────────────────────────────────────────
    let groups = MemoryGroupingEngine::group(&empty_results);

    // ── Assert ───────────────────────────────────────────────────────────────
    assert!(groups.is_empty(), "Empty input must produce zero groups");
    assert_eq!(
        MemoryGroupingEngine::total_items(&groups),
        0,
        "Total items must be 0"
    );
}

#[test]
fn test_ranking_config_default_threshold_invariant() {
    let config = RankingConfig::default();
    assert_eq!(
        config.minimum_score, 6,
        "Default minimum score must match documented threshold (6)"
    );
}
