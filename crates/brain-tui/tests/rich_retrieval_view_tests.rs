//! Unit tests for Milestone A2 Rich Retrieval View widgets (EvidenceCard & ConfidenceBadge).

use brain_domain::retrieval::{
    ConfidenceAssessment, ConfidenceLevel, EvidenceId, EvidenceItem, EvidenceReason,
    StructuredRetrievalExplanation,
};
use brain_domain::{DocumentId, RetrievalWeight, SourceId};
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::widgets::confidence_badge::ConfidenceBadge;
use brain_tui::ui::widgets::evidence_card::EvidenceCard;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_confidence_badge_rendering() {
    let assessment = ConfidenceAssessment::new(0.94);
    assert_eq!(assessment.level, ConfidenceLevel::High);

    let theme = dark_theme();
    let backend = TestBackend::new(40, 3);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let badge = ConfidenceBadge {
                assessment: &assessment,
            };
            badge.render(Rect::new(0, 0, 40, 1), f.buffer_mut(), theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("High Confidence"));
    assert!(buffer_str.contains("0.94"));
}

#[test]
fn test_evidence_card_rendering() {
    let explanation = StructuredRetrievalExplanation {
        reasons: vec![
            EvidenceReason::KeywordMatch {
                term: "SQLite".to_string(),
            },
            EvidenceReason::KeywordMatch {
                term: "FTS5".to_string(),
            },
        ],
        final_rank: 1,
    };

    let item = EvidenceItem {
        id: EvidenceId::new(),
        document: DocumentId::new(),
        source: SourceId("docs/architecture.md".to_string()),
        excerpt: "Hybrid search combines SQLite FTS5 with vector embeddings.".to_string(),
        line_range: Some((182, 241)),
        score: 0.94,
        weight: RetrievalWeight::Critical,
        explanation,
    };

    let theme = dark_theme();
    let backend = TestBackend::new(80, 8);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let card = EvidenceCard {
                item: &item,
                index: 0,
                is_selected: true,
            };
            card.render(Rect::new(0, 0, 80, 8), f.buffer_mut(), theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("Evidence — Score 0.94"));
    assert!(buffer_str.contains("docs/architecture.md"));
    assert!(buffer_str.contains("SQLite • FTS5"));
    assert!(buffer_str.contains("Critical"));
}

#[test]
fn test_exact_score_boundary_mapping() {
    assert_eq!(ConfidenceAssessment::new(1.0).level, ConfidenceLevel::High);
    assert_eq!(ConfidenceAssessment::new(0.85).level, ConfidenceLevel::High);
    assert_eq!(
        ConfidenceAssessment::new(0.849999).level,
        ConfidenceLevel::Medium
    );
    assert_eq!(
        ConfidenceAssessment::new(0.65).level,
        ConfidenceLevel::Medium
    );
    assert_eq!(
        ConfidenceAssessment::new(0.649999).level,
        ConfidenceLevel::Low
    );
    assert_eq!(ConfidenceAssessment::new(0.40).level, ConfidenceLevel::Low);
    assert_eq!(
        ConfidenceAssessment::new(0.399999).level,
        ConfidenceLevel::Uncertain
    );
    assert_eq!(
        ConfidenceAssessment::new(0.0).level,
        ConfidenceLevel::Uncertain
    );
}

#[test]
fn test_grouping_preserves_retrieval_rank_within_tiers() {
    use brain_tui::client::Confidence;
    use brain_tui::ui::command::CommandId;
    use brain_tui::ui::search::types::{SearchResult, SearchResultAction, SearchResultKind};
    use brain_tui::ui::view_models::memory_search_results::MemoryGroupingEngine;

    let make_res = |id: &str, score: i32, conf: Confidence| SearchResult {
        entity_id: id.to_string(),
        title: Some(format!("Item {}", id)),
        subtitle: None,
        kind: SearchResultKind::Knowledge,
        provider_score: score,
        confidence: conf,
        action: SearchResultAction::InvokeCommand(CommandId("dummy")),
    };

    // Upstream retrieval produces: A (High), B (Med), C (High), D (Med), E (High)
    let results = vec![
        make_res("A", 91, Confidence::High),
        make_res("B", 72, Confidence::Medium),
        make_res("C", 94, Confidence::High),
        make_res("D", 61, Confidence::Medium),
        make_res("E", 88, Confidence::High),
    ];

    let groups = MemoryGroupingEngine::group(&results);
    assert_eq!(groups.len(), 2);

    // High tier must preserve exact input sequence: A, C, E
    let high_items: Vec<&str> = groups[0]
        .items
        .iter()
        .map(|i| i.display_title.as_str())
        .collect();
    assert_eq!(high_items, vec!["Item A", "Item C", "Item E"]);

    // Medium tier must preserve exact input sequence: B, D
    let med_items: Vec<&str> = groups[1]
        .items
        .iter()
        .map(|i| i.display_title.as_str())
        .collect();
    assert_eq!(med_items, vec!["Item B", "Item D"]);
}

#[test]
fn test_viewmodel_grouping_determinism() {
    use brain_tui::client::Confidence;
    use brain_tui::ui::command::CommandId;
    use brain_tui::ui::search::types::{SearchResult, SearchResultAction, SearchResultKind};
    use brain_tui::ui::view_models::memory_search_results::MemoryGroupingEngine;

    let make_res = |id: &str, conf: Confidence| SearchResult {
        entity_id: id.to_string(),
        title: Some(format!("Memory {}", id)),
        subtitle: None,
        kind: SearchResultKind::Knowledge,
        provider_score: 80,
        confidence: conf,
        action: SearchResultAction::InvokeCommand(CommandId("dummy")),
    };

    let results = vec![
        make_res("1", Confidence::High),
        make_res("2", Confidence::Medium),
        make_res("3", Confidence::Low),
    ];

    let run1 = MemoryGroupingEngine::group(&results);
    let run2 = MemoryGroupingEngine::group(&results);

    assert_eq!(
        run1, run2,
        "Grouping output must be byte-for-byte identical across runs"
    );
}
