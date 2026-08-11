use brain_tui::state::UiState;
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;

#[test]
fn test_home_welcome_surface_terracotta_border_and_geometry() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let state = UiState::default();
    let theme = Theme::default();
    let renderer = AppRenderer::new();

    terminal
        .draw(|f| {
            renderer.draw(f, Rect::new(0, 0, 80, 24), &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();

    // 1. Top border title at (3..24, 2) is " Claude Code v2.1.226 " in Color::Rgb(215, 119, 87)
    let title_cells: String = (3..=24).map(|x| buf.get(x, 2).symbol()).collect();
    assert_eq!(title_cells, " Claude Code v2.1.226 ");
    assert_eq!(
        buf.get(3, 2).style().fg,
        Some(Color::Rgb(215, 119, 87)),
        "Title prefix must be terracotta RGB(215, 119, 87)"
    );

    // 2. Left border (1, y) is │ in Color::Rgb(215, 119, 87)
    for y in 3..10 {
        assert_eq!(buf.get(1, y).symbol(), "│");
        assert_eq!(
            buf.get(1, y).style().fg,
            Some(Color::Rgb(215, 119, 87)),
            "Left border at (1, {}) must be terracotta RGB(215, 119, 87)",
            y
        );
    }

    // 3. Vertical divider at (47, y) is │ in Color::Rgb(80, 80, 80)
    for y in 3..10 {
        assert_eq!(buf.get(47, y).symbol(), "│");
        assert_eq!(
            buf.get(47, y).style().fg,
            Some(Color::Rgb(80, 80, 80)),
            "Vertical divider at (47, {}) must be subtle RGB(80, 80, 80)",
            y
        );
    }

    // 4. Right rail interior spans x = 48..77
    let rail_text = (3..10)
        .map(|y| (48..78).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rail_text.contains("Tips for getting started"));
    assert!(rail_text.contains("What's new"));

    // 5. Right border edge (78, y) is │ in Color::Rgb(215, 119, 87)
    for y in 3..10 {
        assert_eq!(buf.get(78, y).symbol(), "│");
        assert_eq!(
            buf.get(78, y).style().fg,
            Some(Color::Rgb(215, 119, 87)),
            "Right border edge at (78, {}) must be terracotta RGB(215, 119, 87)",
            y
        );
    }

    // 6. Ambient status line at y = 19 is "● xhigh · /effort"
    let status_line: String = (0..80).map(|x| buf.get(x, 19).symbol()).collect();
    assert!(
        status_line.contains("● xhigh · /effort"),
        "Ambient status line at y=19 must contain '● xhigh · /effort', got: '{}'",
        status_line
    );
}

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

