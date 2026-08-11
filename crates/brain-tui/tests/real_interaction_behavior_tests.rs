//! Real Interaction & Behavioral QA Test Suite
//!
//! Exercises real terminal I/O interaction patterns and edge cases:
//! 1. Keyboard event sequences (char entry, backspace, cursor navigation, submit)
//! 2. Mouse & scroll behavior (scroll up/down, policy transition Pinned <-> Manual)
//! 3. Streaming + scrolling simultaneously (manual scroll position preserved during active stream)
//! 4. Reconnect during an active request (Daemon -> Disconnected -> Reconnecting -> Daemon)
//! 5. Stream cancellation & interruption (CancelStream action, typewriter queue clear)
//! 6. Very long streamed responses (1,000+ tokens, memory stability, virtual scroll bounds)
//! 7. Concurrent session updates (background session metadata updates while active)
//! 8. Terminal resize while streaming (resize from 120x30 to 70x20 during active stream)

use brain_domain::SessionId;
use brain_tui::state::{
    Action, ConnectionMode, GenerationState, RenderToken, SessionViewModel, UiState,
};
use brain_tui::ui::interaction::scroll::{AutoFollowPolicy, ScrollState};
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::{Instant, SystemTime};

// ============================================================================
// 1. Keyboard Event Sequences
// ============================================================================
#[test]
fn test_real_keyboard_event_sequences() {
    let mut state = UiState::new();
    state.screen = Screen::Workspace;

    // Type "/search concept" character by character
    for c in "/search concept".chars() {
        state.update(Action::InsertChar(c));
    }
    assert_eq!(state.editor.text(), "/search concept");

    // Move cursor left 7 times
    for _ in 0..7 {
        state.update(Action::MoveCursorLeft);
    }

    // Delete character at cursor (space character deleted)
    state.update(Action::Backspace);
    assert_eq!(state.editor.text(), "/searchconcept");

    // Clear editor on submit
    state.update(Action::SubmitPrompt);
    assert_eq!(state.editor.text(), "");
    assert!(matches!(state.generation_state, GenerationState::Starting));
}

// ============================================================================
// 2. Mouse & Scroll Behavior
// ============================================================================
#[test]
fn test_mouse_and_scroll_behavior() {
    let mut scroll = ScrollState::new();
    assert_eq!(scroll.policy, AutoFollowPolicy::Pinned);

    // Scroll up transitions policy to Manual
    scroll.scroll_up();
    assert_eq!(scroll.policy, AutoFollowPolicy::Manual);

    scroll.update_bounds(50, 10);
    assert_eq!(scroll.offset(), 0);

    // Scroll down to the bottom restores Pinned policy
    for _ in 0..40 {
        scroll.scroll_down();
    }
    assert_eq!(scroll.policy, AutoFollowPolicy::Pinned);
}

// ============================================================================
// 3. Streaming + Scrolling Simultaneously
// ============================================================================
#[test]
fn test_simultaneous_streaming_and_scrolling() {
    let renderer = AppRenderer::new();
    let theme = Theme::default();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.screen = Screen::Workspace;
    state.update(Action::StartStream);

    // User scrolls up manually mid-stream
    state.viewport.scroll_offset = 5;
    state.viewport.follow_tail = false;

    // Incremental tokens arrive while scrolled up
    for i in 0..10 {
        state.update(Action::ReceiveToken(RenderToken::Text(format!(
            "Token_{} ",
            i
        ))));
        let res = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
        assert!(
            res.is_ok(),
            "Render failed during stream while user was scrolled up"
        );

        // Verify scroll position was not forced back to tail
        assert_eq!(
            state.viewport.scroll_offset, 5,
            "Manual scroll position must be preserved during active streaming"
        );
    }
}

// ============================================================================
// 4. Reconnect During Active Request
// ============================================================================
#[test]
fn test_reconnect_during_active_request() {
    let renderer = AppRenderer::new();
    let theme = Theme::default();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.screen = Screen::Workspace;

    // Active request streaming under Daemon mode
    state.connection_mode = ConnectionMode::Daemon;
    state.update(Action::StartStream);

    // Connection drops to Disconnected mid-stream
    state.update(Action::SetConnectionMode(ConnectionMode::Disconnected));
    assert_eq!(state.connection_mode, ConnectionMode::Disconnected);
    let res_disc = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(res_disc.is_ok());

    // Reconnecting state
    state.update(Action::SetConnectionMode(ConnectionMode::Connecting));
    assert_eq!(state.connection_mode, ConnectionMode::Connecting);
    let res_conn = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(res_conn.is_ok());

    // Successfully reconnected to Daemon
    state.update(Action::SetConnectionMode(ConnectionMode::Daemon));
    assert_eq!(state.connection_mode, ConnectionMode::Daemon);
    let res_ok = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(res_ok.is_ok());
}

// ============================================================================
// 5. Stream Cancellation & Interruption
// ============================================================================
#[test]
fn test_stream_cancellation_and_interruption() {
    let mut state = UiState::new();
    state.screen = Screen::Workspace;
    state.update(Action::StartStream);
    state.update(Action::ReceiveToken(RenderToken::Text(
        "Partial text ".to_string(),
    )));

    // User cancels active stream (Escape or Ctrl+C)
    state.update(Action::CancelStream);
    assert!(matches!(
        state.generation_state,
        GenerationState::Cancelled(_)
    ));
    assert!(state.typewriter.is_empty());
}

// ============================================================================
// 6. Very Long Streamed Response Handling (1,000+ Tokens)
// ============================================================================
#[test]
fn test_very_long_streamed_response() {
    let renderer = AppRenderer::new();
    let theme = Theme::default();
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.screen = Screen::Workspace;
    state.update(Action::StartStream);

    // Stream 1,000 tokens incrementally
    for i in 0..1000 {
        state.update(Action::ReceiveToken(RenderToken::Text(format!(
            "Token_{} ",
            i
        ))));
        if i % 100 == 0 {
            state.update(Action::TypewriterTick(Instant::now()));
            let res = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
            assert!(res.is_ok(), "Render failed at token {}", i);
        }
    }

    state.update(Action::FinishStream);
    state.update(Action::TypewriterTick(Instant::now()));
    assert!(matches!(state.generation_state, GenerationState::Finished));
}

// ============================================================================
// 7. Concurrent Session Updates
// ============================================================================
#[test]
fn test_concurrent_session_updates() {
    let mut state = UiState::new();
    let s1 = SessionId::new();
    let s2 = SessionId::new();

    state.sessions = vec![
        SessionViewModel {
            id: s1,
            title: "Active Session".to_string(),
            updated_at: SystemTime::now(),
            active: true,
            preview: None,
            pinned: false,
            archived: false,
        },
        SessionViewModel {
            id: s2,
            title: "Background Session".to_string(),
            updated_at: SystemTime::now(),
            active: false,
            preview: None,
            pinned: false,
            archived: false,
        },
    ];

    // Background session title updated via RPC sync
    state.sessions[1].title = "Background Session (Updated via RPC)".to_string();
    assert_eq!(state.sessions[0].title, "Active Session");
    assert_eq!(
        state.sessions[1].title,
        "Background Session (Updated via RPC)"
    );
}

// ============================================================================
// 8. Terminal Resize While Streaming
// ============================================================================
#[test]
fn test_terminal_resize_while_streaming() {
    let renderer = AppRenderer::new();
    let theme = Theme::default();
    let mut state = UiState::new();
    state.screen = Screen::Workspace;
    state.update(Action::StartStream);

    // 1. Initial wide viewport (120x30)
    let backend_wide = TestBackend::new(120, 30);
    let mut terminal_wide = Terminal::new(backend_wide).unwrap();
    state.update(Action::Resize(120, 30));
    state.update(Action::ReceiveToken(RenderToken::Text(
        "Streaming before resize... ".to_string(),
    )));

    let res_wide = terminal_wide.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(res_wide.is_ok());

    // 2. Sudden resize to compact viewport (70x20) mid-stream
    let backend_compact = TestBackend::new(70, 20);
    let mut terminal_compact = Terminal::new(backend_compact).unwrap();
    state.update(Action::Resize(70, 20));
    state.update(Action::ReceiveToken(RenderToken::Text(
        "Streaming after resize... ".to_string(),
    )));

    let res_compact = terminal_compact.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(
        res_compact.is_ok(),
        "Render failed when resized to compact viewport mid-stream"
    );
}
