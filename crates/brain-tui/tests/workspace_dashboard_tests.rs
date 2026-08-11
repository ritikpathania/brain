use brain_tui::state::UiState;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_workspace_dashboard_full_width_layout() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let state = UiState::default();
    let theme = Theme::default();

    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            brain_tui::ui::widgets::workspace_dashboard::draw(f, area, &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let text = (0..20)
        .map(|y| (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    // Positive Assertions
    assert!(text.contains("Claude Code v2.1.226"), "Must contain integrated version header");
    assert!(text.contains("awaiting input"), "Must contain awaiting input status");
    assert!(
        text.contains("Your conversation moved to the background"),
        "Must contain background banner text"
    );
    assert!(text.contains("Needs input"), "Must contain Needs input section");
    assert!(text.contains("Completed"), "Must contain Completed section");

    // Negative Assertions: 2-column sidebar split line must NOT exist
    for y in 0..20 {
        assert_ne!(
            buf.get(22, y).symbol(),
            "│",
            "Vertical sidebar divider must not exist at col 22 for y={}",
            y
        );
    }
    assert!(
        !text.contains("Sessions (Active)"),
        "Legacy Sessions (Active) text must not exist"
    );
}
