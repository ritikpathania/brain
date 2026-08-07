use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use brain_tui::state::{UiState, Action, FocusRegion, PinnedNode};
use brain_domain::{NodeId, NodeKind};
use uuid::Uuid;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn test_matrix_viewport_width_invariants_no_panic() {
    let widths = [40, 60, 80, 120];
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    for width in widths {
        let backend = TestBackend::new(width, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = UiState::new();
        state.terminal_width = width;
        state.recalculate_viewport();

        // Must draw cleanly without buffer overflow or panic
        let res = terminal.draw(|f| {
            renderer.draw(f, f.size(), &state, &theme);
        });
        assert!(res.is_ok(), "Failed render at width {}", width);
    }
}

#[test]
fn test_matrix_narrow_viewport_sidebar_collapse() {
    let mut state = UiState::new();
    state.terminal_width = 50; // Ultra-narrow (<80 cols)
    state.recalculate_viewport();

    state.update(Action::ToggleFocus);
    assert_eq!(state.focus, FocusRegion::Editor, "Sidebar should not be focusable in ultra-narrow mode");
}

#[test]
fn test_matrix_keyboard_modifier_routing() {
    let mut state = UiState::new();
    state.pinned_nodes.push(PinnedNode {
        node_id: NodeId(Uuid::new_v4()),
        label: "Alpha".to_string(),
        node_type: NodeKind::Concept,
        pinned_at: 0,
    });
    assert!(!state.submit_with_workspace);

    // Toggle Workspace Submit mode (Alt+W action)
    state.update(Action::ToggleSubmitWithWorkspace);
    assert!(state.submit_with_workspace);

    // Reset flag after submit
    state.update(Action::ResetSubmitWithWorkspace);
    assert!(!state.submit_with_workspace);
}
