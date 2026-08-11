mod common;

use brain_domain::ExecutionId;
use brain_tui::ui::render::{IconSet, RenderContext};
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::widgets::reasoning_progress::{
    ReasoningProgressState, ReasoningProgressWidget, StepStatus,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

#[test]
fn test_new_execution_resets_progress_state() {
    let exec_a = ExecutionId::new();
    let exec_b = ExecutionId::new();
    let mut state = ReasoningProgressState::new();

    // Run A
    state.reset(exec_a);
    state.update_stage(
        "retrieval",
        StepStatus::Completed(Some("Retrieved 18 memories".to_string())),
    );
    state.on_token();
    assert!(state.is_collapsed);
    assert_eq!(state.execution_id, Some(exec_a));

    // Start new Execution B -> reset clears steps & un-collapses
    state.reset(exec_b);
    assert_eq!(state.execution_id, Some(exec_b));
    assert!(!state.is_collapsed);
    assert_eq!(state.steps[0].status, StepStatus::Pending);
    assert_eq!(state.terminal_error, None);
}

#[test]
fn test_failed_execution_preserves_terminal_progress() {
    let exec = ExecutionId::new();
    let mut state = ReasoningProgressState::new();
    state.reset(exec);

    state.update_stage(
        "retrieval",
        StepStatus::Completed(Some("18 memories".to_string())),
    );
    state.update_stage("synthesis", StepStatus::Failed("Model timeout".to_string()));

    // Emit error before token
    state.on_error("Model timeout".to_string());

    // Token arrival does NOT collapse when terminal_error is set
    state.on_token();

    assert!(!state.is_collapsed);
    assert_eq!(state.terminal_error, Some("Model timeout".to_string()));
}

#[test]
fn test_reasoning_progress_widget_rendering() {
    let exec = ExecutionId::new();
    let mut state = ReasoningProgressState::new();
    state.reset(exec);
    state.update_stage(
        "retrieval",
        StepStatus::Completed(Some("Retrieved 18 memories".to_string())),
    );
    state.update_stage("synthesis", StepStatus::Active);

    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext {
        theme,
        icons: &icons,
        capabilities,
        tick: 5,
    };

    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);

    let widget = ReasoningProgressWidget {
        state: &state,
        ctx: &ctx,
    };
    widget.render(area, &mut buf);

    // Check rendered lines contain completed retrieval checkmark and synthesis step
    let text = format!("{:?}", buf);
    assert!(text.contains("Retrieved 18 memories"));
    assert!(text.contains("Synthesizing response"));
}

#[test]
fn test_collapsed_reasoning_progress_renders_zero_height() {
    let exec = ExecutionId::new();
    let mut state = ReasoningProgressState::new();
    state.reset(exec);
    state.on_token(); // Collapses progress widget

    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext {
        theme,
        icons: &icons,
        capabilities,
        tick: 0,
    };

    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);

    let widget = ReasoningProgressWidget {
        state: &state,
        ctx: &ctx,
    };
    widget.render(area, &mut buf);

    // Buffer remains completely empty when collapsed
    for cell in buf.content() {
        assert_eq!(cell.symbol(), " ");
    }
}
