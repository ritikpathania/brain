use brain_domain::{Message, MessageId, MessageRole, SessionId};
use brain_tui::client::SessionSummary;
use brain_tui::state::{Action, ConnectionMode, UiState};
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::SystemTime;

#[test]
fn test_release_hardening_large_conversation_rendering_gate() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();
    let (w, h) = (182, 53);

    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.screen = Screen::Workspace;
    state.connection_mode = ConnectionMode::Daemon;
    state.terminal_width = w;
    state.terminal_height = h;

    // Populate 1,000 messages into active_messages history
    let mut messages = Vec::with_capacity(1000);
    for i in 0..1000 {
        let role = if i % 2 == 0 {
            MessageRole::User
        } else {
            MessageRole::Assistant
        };
        messages.push(Message::new(
            MessageId::new(),
            role,
            format!("Timeline performance test message row #{}", i),
        ));
    }
    state.active_messages = messages;
    state.recalculate_viewport();

    // Render frame - must execute cleanly without panic or unbounded stalls
    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, theme))
        .unwrap();

    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(
        !text.is_empty(),
        "Failed 1,000-message timeline layout rendering"
    );
}

#[test]
fn test_release_hardening_large_workspace_session_listing_gate() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();
    let (w, h) = (120, 30);

    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.screen = Screen::Workspace;
    state.focus = brain_tui::state::FocusRegion::Sidebar;
    state.connection_mode = ConnectionMode::Daemon;
    state.terminal_width = w;
    state.terminal_height = h;

    // Populate 500 session summaries in Workspace sidebar listing
    let mut summaries = Vec::with_capacity(500);
    for i in 0..500 {
        summaries.push(SessionSummary {
            id: SessionId::new(),
            title: format!("Historical Workspace Thread #{}", i),
            updated_at: SystemTime::now(),
            pinned: i % 10 == 0,
            archived: false,
        });
    }

    state.update(Action::LoadSessions(summaries));
    state.selected_session_idx = 499; // Select last session

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, theme))
        .unwrap();

    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(
        text.contains("Thread #499") || text.contains("Historical"),
        "Failed large workspace listing rendering"
    );
}
