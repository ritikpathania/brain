use brain_tui::ui::command::palette::CommandPaletteState;
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::palette::CommandPaletteWidget;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_command_palette_3_column_reconstruction() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();
    let mut state = CommandPaletteState::new();
    state.open = true;

    let area = Rect::new(0, 0, 80, 20);

    terminal
        .draw(|f| {
            let widget = CommandPaletteWidget::new(&state, &theme);
            widget.draw(f, area);
        })
        .unwrap();

    let buf = terminal.backend().buffer();

    let mut full_text = String::new();
    for y in 0..20 {
        let line = (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>();
        full_text.push_str(&line);
        full_text.push('\n');
    }

    // Positive assertions: Verify command names exist ("/session", "/help")
    assert!(
        full_text.contains("/session") || full_text.contains("/help"),
        "Command palette 3-column layout must contain command names (/session or /help). Rendered buffer:\n{}",
        full_text
    );

    // Positive assertions: Verify categories exist ("Session", "System")
    assert!(
        full_text.contains("Session") || full_text.contains("System"),
        "Command palette 3-column layout must contain category names (Session or System). Rendered buffer:\n{}",
        full_text
    );

    // Positive assertions: Verify descriptions exist
    assert!(
        full_text.contains("reasoning session")
            || full_text.contains("shortcuts")
            || full_text.contains("memory"),
        "Command palette 3-column layout must contain command descriptions. Rendered buffer:\n{}",
        full_text
    );
}
