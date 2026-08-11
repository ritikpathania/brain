//! Integration test suite for Phase C (Reflection & Memory Stewardship).

use brain_domain::reflection::{
    FindingKind, KnowledgeFactInput, RecommendationKind, ReflectionEngine, ResolutionStatus,
    ResolutionStrategy, StewardshipResolution,
};
use brain_domain::SourceId;
use brain_tui::ui::navigation::modal::Modal;
use brain_tui::ui::navigation::screen::Screen;
use brain_tui::ui::navigation::stack::NavigationStack;
use brain_tui::ui::screens::reflection::{ReflectionScreen, ReflectionScreenState};
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_pure_reflection_engine_analysis_and_recommendation() {
    let engine = ReflectionEngine::new();
    let inputs = vec![
        KnowledgeFactInput {
            source: SourceId("doc_a.md".to_string()),
            content: "SQLite FTS5 provides fast full-text indexing.".to_string(),
        },
        KnowledgeFactInput {
            source: SourceId("doc_b.md".to_string()),
            content: "SQLite FTS5 provides fast full-text indexing.".to_string(),
        },
    ];

    let report = engine.analyze(&inputs);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, FindingKind::Duplication);
    assert_eq!(report.recommendations.len(), 1);
    assert_eq!(report.recommendations[0].kind, RecommendationKind::Merge);
}

#[test]
fn test_resolution_revert_lifecycle_invariants() {
    let engine = ReflectionEngine::new();
    let inputs = vec![
        KnowledgeFactInput {
            source: SourceId("doc_a.md".to_string()),
            content: "Same content".to_string(),
        },
        KnowledgeFactInput {
            source: SourceId("doc_b.md".to_string()),
            content: "Same content".to_string(),
        },
    ];

    let mut report = engine.analyze(&inputs);
    let finding_id = report.findings[0].id;
    let rec_id = report.recommendations[0].id;

    let mut resolution =
        StewardshipResolution::new(finding_id, Some(rec_id), ResolutionStrategy::Manual);
    assert_eq!(resolution.status, ResolutionStatus::Applied);

    resolution.revert();
    assert_eq!(resolution.status, ResolutionStatus::Reverted);

    report.add_resolution(resolution);
    assert_eq!(report.resolutions.len(), 1);
}

#[test]
fn test_reflection_screen_dashboard_rendering() {
    let engine = ReflectionEngine::new();
    let inputs = vec![
        KnowledgeFactInput {
            source: SourceId("doc_a.md".to_string()),
            content: "Contradiction sample text".to_string(),
        },
        KnowledgeFactInput {
            source: SourceId("doc_b.md".to_string()),
            content: "Contradiction sample text".to_string(),
        },
    ];

    let report = engine.analyze(&inputs);
    let state = ReflectionScreenState {
        report,
        selected_index: 0,
    };

    let theme = dark_theme();
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let screen = ReflectionScreen { state: &state };
            screen.render(Rect::new(0, 0, 120, 30), f.buffer_mut(), theme);
        })
        .unwrap();

    let buf_str = format!("{:?}", terminal.backend().buffer());
    assert!(buf_str.contains("Stewardship Summary"));
    assert!(buf_str.contains("Stewardship Findings"));
    assert!(buf_str.contains("Finding Details & Evidence"));
}

#[test]
fn test_reflection_deep_navigation_stack() {
    let mut stack = NavigationStack::new(Screen::Home);

    stack.push(Screen::Workspace);
    assert_eq!(stack.current(), Screen::Workspace);

    stack.push(Screen::Reflection);
    assert_eq!(stack.current(), Screen::Reflection);

    let modal = Modal::DocumentInspector;
    assert_eq!(modal.title(), "Document Inspector");

    assert_eq!(stack.pop(), Some(Screen::Reflection));
    assert_eq!(stack.current(), Screen::Workspace);
}
