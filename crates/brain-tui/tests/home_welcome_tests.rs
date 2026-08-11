use brain_tui::state::UiState;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_home_welcome_surface_geometry_and_components() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let state = UiState::default();
    let theme = Theme::default();

    terminal
        .draw(|f| {
            let surface_rect = Rect::new(1, 2, 78, 9);
            brain_tui::ui::widgets::home_welcome::draw(f, surface_rect, &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();

    // Assert top border box starts at y=2 with integrated title
    let row2 = (0..80).map(|x| buf.get(x, 2).symbol()).collect::<String>();
    assert!(row2.contains("Claude Code v2.1.226"), "Row 2 must contain integrated title");
    assert!(row2.contains("┌"), "Row 2 must contain top-left border corner");
    assert!(row2.contains("┐"), "Row 2 must contain top-right border corner");

    // Assert vertical divider at x=47
    assert_eq!(buf.get(47, 3).symbol(), "│");
    assert_eq!(buf.get(47, 4).symbol(), "│");
    assert_eq!(buf.get(47, 5).symbol(), "│");

    // Assert Right Rail Headers
    let rail_text = (3..9)
        .map(|y| (48..78).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rail_text.contains("Tips for getting started"));
    assert!(rail_text.contains("What's new"));

    // Negative assertions: legacy Brain UI text MUST NOT exist
    let full_text = (0..24)
        .map(|y| (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!full_text.contains("System Status"), "System Status must not exist");
    assert!(!full_text.contains("Context"), "Context telemetry must not exist");
}
