use brain_tui::state::{FocusRegion, UiState};
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn test_master_home_80x24_cell_buffer_reconstruction() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Home;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, &theme))
        .unwrap();
    let buf = terminal.backend().buffer();

    // 1. Surface Border Box at y=2
    assert_eq!(buf.get(1, 2).symbol(), "┌");
    assert_eq!(buf.get(78, 2).symbol(), "┐");

    // 2. Integrated Title
    let title_line = (0..80).map(|x| buf.get(x, 2).symbol()).collect::<String>();
    assert!(
        title_line.contains("Claude Code v2.1.226"),
        "Title line at y=2 must contain 'Claude Code v2.1.226'"
    );

    // 3. Vertical Divider at x=47
    assert_eq!(buf.get(47, 4).symbol(), "│");

    // 4. Ambient Status at y=19
    let ambient_line = (0..80).map(|x| buf.get(x, 19).symbol()).collect::<String>();
    assert!(
        ambient_line.contains("● xhigh · /effort"),
        "Ambient status line at y=19 must contain '● xhigh · /effort'"
    );

    // 5. Prompt Prefix at y=21
    assert_eq!(buf.get(0, 21).symbol(), "❯");

    // 6. Quiet status footer at y=23
    let footer_line = (0..80).map(|x| buf.get(x, 23).symbol()).collect::<String>();
    assert!(
        footer_line.contains("manual mode on") || footer_line.contains("? for shortcuts"),
        "Row 23 must contain quiet status footer text"
    );

    // 7. Negative Assertions
    let full_screen = (0..24)
        .map(|y| (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!full_screen.contains("System Status"));
    assert!(!full_screen.contains("Context"));
}

#[test]
fn test_master_workspace_80x24_cell_buffer_reconstruction() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Workspace;
    state.focus = FocusRegion::Sidebar;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, &theme))
        .unwrap();
    let buf = terminal.backend().buffer();

    // Must be full-width task table, NOT a 22-column sidebar split
    assert_ne!(
        buf.get(22, 5).symbol(),
        "│",
        "Vertical sidebar divider must not exist in Workspace mode"
    );

    let full_screen = (0..24)
        .map(|y| (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(full_screen.contains("Needs input"));
    assert!(full_screen.contains("Completed"));
    assert!(!full_screen.contains("Sessions (Active)"));
}

#[test]
fn test_master_home_viewport_100x26() {
    let backend = TestBackend::new(100, 26);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Home;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, &theme))
        .unwrap();
    let buf = terminal.backend().buffer();

    let title_line = (0..100).map(|x| buf.get(x, 2).symbol()).collect::<String>();
    assert!(title_line.contains("Claude Code v2.1.226"));

    let footer_line = (0..100).map(|x| buf.get(x, 25).symbol()).collect::<String>();
    assert!(footer_line.contains("manual mode on") || footer_line.contains("? for shortcuts"));
}

#[test]
fn test_master_home_viewport_120x30() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Home;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, &theme))
        .unwrap();
    let buf = terminal.backend().buffer();

    let title_line = (0..120).map(|x| buf.get(x, 2).symbol()).collect::<String>();
    assert!(title_line.contains("Claude Code v2.1.226"));

    // At 120x30, prompt is anchored at 67% (y=20), prompt input row is y=21
    assert_eq!(buf.get(0, 21).symbol(), "❯");

    let footer_line = (0..120).map(|x| buf.get(x, 29).symbol()).collect::<String>();
    assert!(footer_line.contains("manual mode on") || footer_line.contains("? for shortcuts"));
}

#[test]
fn test_master_home_viewport_182x53() {
    let backend = TestBackend::new(182, 53);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Home;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, &theme))
        .unwrap();
    let buf = terminal.backend().buffer();

    let title_line = (0..182).map(|x| buf.get(x, 2).symbol()).collect::<String>();
    assert!(title_line.contains("Claude Code v2.1.226"));

    // At 182x53, prompt is anchored at 67% (y=35), prompt input row is y=36
    assert_eq!(buf.get(0, 36).symbol(), "❯");

    let footer_line = (0..182).map(|x| buf.get(x, 52).symbol()).collect::<String>();
    assert!(footer_line.contains("manual mode on") || footer_line.contains("? for shortcuts"));
}
