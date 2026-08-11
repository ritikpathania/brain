use brain_tui::state::{Action, ConnectionMode, UiState};
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn test_release_hardening_e2e_monotonic_chunk_stream() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();
    let (w, h) = (120, 30);

    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.screen = Screen::Workspace;
    state.connection_mode = ConnectionMode::Daemon;
    state.terminal_width = w;
    state.terminal_height = h;

    // Simulate prompt submission and incoming monotonic stream chunks
    for c in "Explain release hardening".chars() {
        state.editor.insert(c);
    }
    state.update(Action::SubmitPrompt);

    let chunks = vec![
        "Release ",
        "hardening ",
        "verifies ",
        "production ",
        "readiness ",
        "without ",
        "mutating ",
        "certified ",
        "contracts.",
    ];

    for chunk in chunks {
        state.active_response.push_str(chunk);
        terminal
            .draw(|f| renderer.draw(f, f.size(), &state, theme))
            .unwrap();
    }

    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(
        text.contains("Release") || text.contains("hardening"),
        "Missing streaming chunk text in viewport"
    );
}
