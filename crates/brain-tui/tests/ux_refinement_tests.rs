use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use brain_tui::state::{UiState, Action};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn test_ux_refinement_full_integration_suite() {
    let renderer = AppRenderer::new();
    let mut state = UiState::new();
    let theme = Theme::default();

    // 1. Verify rich autocomplete metadata
    state.editor.set_text("/m");
    let suggestions = state.get_slash_suggestions();
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].name, "/memory");
    assert_eq!(suggestions[0].category, "Graph");

    // 2. Verify PresentationModel virtual scroll engine
    state.viewport.scroll_offset = 10;
    let model = state.presentation_model(50, 20);
    assert_eq!(model.visible_rows.len(), 20);
    assert_eq!(model.scroll_indicator, "Showing 11-30 of 50");

    // 3. Verify collapsible result groups
    state.update(Action::ToggleGroupExpand(0));
    assert!(state.is_group_collapsed(0));

    // 4. Verify footer draw across wide & compact viewports without panic
    for width in [120, 60] {
        let backend = TestBackend::new(width, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        state.terminal_width = width;
        state.recalculate_viewport();
        let res = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
        assert!(res.is_ok(), "Failed status footer render at width {}", width);
    }
}
