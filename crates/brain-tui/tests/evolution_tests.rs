//! Integration test suite for Phase D (Knowledge Evolution).

use brain_domain::evolution::{
    EvolutionEngine, EvolutionExecution, ExecutionResult, Priority, ProposalStatus,
};
use brain_domain::reflection::finding::{FindingKind, StewardshipFinding};
use brain_domain::reflection::report::StewardshipReport;
use brain_domain::retrieval::ConfidenceAssessment;
use brain_tui::ui::navigation::modal::Modal;
use brain_tui::ui::navigation::screen::Screen;
use brain_tui::ui::navigation::stack::NavigationStack;
use brain_tui::ui::screens::evolution::{EvolutionScreen, EvolutionScreenState};
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_pure_evolution_engine_planning_and_priority_grouping() {
    let engine = EvolutionEngine::new();
    let mut report = StewardshipReport::new();

    let finding = StewardshipFinding::new(
        FindingKind::Duplication,
        "Duplicate SQLite Notes",
        "Doc A and Doc B have identical SQLite FTS5 details",
        vec![],
        ConfidenceAssessment::new(0.96),
    );

    report.add_finding(finding);
    let plan = engine.plan(&report);

    assert_eq!(plan.proposals.len(), 1);
    assert_eq!(plan.proposals[0].priority, Priority::High);

    let high_prio = plan.proposals_by_priority(Priority::High);
    assert_eq!(high_prio.len(), 1);
}

#[test]
fn test_evolution_execution_and_rollback_invariants() {
    let engine = EvolutionEngine::new();
    let mut report = StewardshipReport::new();

    let finding = StewardshipFinding::new(
        FindingKind::Duplication,
        "Duplicate Concept",
        "Description",
        vec![],
        ConfidenceAssessment::new(0.90),
    );

    report.add_finding(finding);
    let mut plan = engine.plan(&report);
    let prop_id = plan.proposals[0].id;

    plan.proposals[0].approve();
    assert_eq!(plan.proposals[0].status, ProposalStatus::Approved);

    let mut exec = EvolutionExecution::new_success(prop_id);
    assert_eq!(exec.result, ExecutionResult::Success);

    exec.rollback();
    assert_eq!(exec.result, ExecutionResult::RolledBack);
}

#[test]
fn test_evolution_screen_layout_rendering() {
    let engine = EvolutionEngine::new();
    let mut report = StewardshipReport::new();

    let finding = StewardshipFinding::new(
        FindingKind::Duplication,
        "Duplicate Concept",
        "Description",
        vec![],
        ConfidenceAssessment::new(0.90),
    );

    report.add_finding(finding);
    let plan = engine.plan(&report);

    let state = EvolutionScreenState {
        plan,
        selected_index: 0,
    };

    let theme = dark_theme();
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let screen = EvolutionScreen { state: &state };
            screen.render(Rect::new(0, 0, 120, 30), f.buffer_mut(), theme);
        })
        .unwrap();

    let buf_str = format!("{:?}", terminal.backend().buffer());
    assert!(buf_str.contains("Planned Evolution Proposals"));
    assert!(buf_str.contains("Semantic Graph Transformation Diff"));
}

#[test]
fn test_evolution_deep_navigation_stack() {
    let mut stack = NavigationStack::new(Screen::Home);

    stack.push(Screen::Workspace);
    assert_eq!(stack.current(), Screen::Workspace);

    stack.push(Screen::Evolution);
    assert_eq!(stack.current(), Screen::Evolution);

    let modal = Modal::DocumentInspector;
    assert_eq!(modal.title(), "Document Inspector");

    assert_eq!(stack.pop(), Some(Screen::Evolution));
    assert_eq!(stack.current(), Screen::Workspace);
}
