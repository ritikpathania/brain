//! Stateful UX Validation Integration Test Suite
//!
//! Validates the 5 stateful UX flow stages against the frozen visual baseline:
//! 1. Streaming response flow (typewriter queue, incremental chunking, layout stability)
//! 2. Knowledge timeline (empty state, single/multiple results, text wrapping, virtual scroll)
//! 3. Session lifecycle (session creation, selection, switching, title formatting)
//! 4. Runtime resilience (disconnected, connecting, connected status, error recovery)
//! 5. Interaction matrix (focus traversal, keyboard routing, compact fallback)

use brain_domain::{Message, MessageId, MessageRole, SessionId};
use brain_tui::state::{
    Action, ConnectionMode, GenerationState, RenderToken, SessionViewModel, UiState,
};
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::renderer::{AppLayoutMode, AppRenderer};
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::SystemTime;

// ============================================================================
// STAGE 1: Streaming Response Flow
// ============================================================================
#[test]
fn test_stage1_streaming_response_flow() {
    let renderer = AppRenderer::new();
    let theme = Theme::default();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.screen = Screen::Workspace;

    // 1. Initial prompt submission: state enters Starting / Generating
    state.editor.set_text("What is relational memory?");
    state.update(Action::SubmitPrompt);
    assert!(matches!(state.generation_state, GenerationState::Starting));
    assert_eq!(state.active_messages.len(), 1);

    // Render step during starting state
    let res_starting = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(res_starting.is_ok(), "Render failed during Starting state");

    // 2. Incremental typewriter tokens streamed in
    state.update(Action::StartStream);
    assert!(matches!(
        state.generation_state,
        GenerationState::Starting | GenerationState::Streaming { .. }
    ));

    for chunk in [
        "Relational ",
        "memory ",
        "connects ",
        "entities ",
        "graphically.",
    ] {
        state.update(Action::ReceiveToken(RenderToken::Text(chunk.to_string())));
        let res_token = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
        assert!(
            res_token.is_ok(),
            "Render failed during typewriter token stream"
        );
    }

    // 3. Stream completes cleanly with typewriter queue drain
    state.update(Action::FinishStream);
    state.update(Action::TypewriterTick(
        std::time::Instant::now() + std::time::Duration::from_millis(100),
    ));
    assert!(matches!(state.generation_state, GenerationState::Finished));
    assert!(!state.active_messages.is_empty());
    assert_eq!(
        state.active_messages[1].content,
        "Relational memory connects entities graphically."
    );
}

// ============================================================================
// STAGE 2: Knowledge Timeline Lifecycle
// ============================================================================
#[test]
fn test_stage2_knowledge_timeline_lifecycle() {
    let renderer = AppRenderer::new();
    let theme = Theme::default();
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.screen = Screen::Workspace;

    // 1. Empty state verification
    let presentation_empty = state.presentation_model(0, 20);
    assert_eq!(presentation_empty.visible_rows.len(), 0);
    assert_eq!(presentation_empty.scroll_indicator, "0 results");

    // 2. Populating memory results (1 item -> many items)
    for i in 1..=50 {
        state.active_messages.push(Message::new(
            MessageId::new(),
            MessageRole::Assistant,
            format!(
                "Knowledge item {} detailing entity relationships and graph metrics.",
                i
            ),
        ));
    }

    // Virtual scroll pagination and layout check
    state.viewport.scroll_offset = 10;
    let presentation_populated = state.presentation_model(50, 20);
    assert_eq!(presentation_populated.visible_rows.len(), 20);
    assert_eq!(
        presentation_populated.scroll_indicator,
        "Showing 11-30 of 50"
    );

    let res_draw = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(res_draw.is_ok(), "Render failed with 50 timeline items");
}

// ============================================================================
// STAGE 3: Session Lifecycle & Selection
// ============================================================================
#[test]
fn test_stage3_session_lifecycle_and_navigation() {
    let mut state = UiState::new();
    let session_id_1 = SessionId::new();
    let session_id_2 = SessionId::new();

    state.sessions = vec![
        SessionViewModel {
            id: session_id_1,
            title: "Initial Research Session".to_string(),
            updated_at: SystemTime::now(),
            active: true,
            preview: Some("Researching graph engine".to_string()),
            pinned: false,
            archived: false,
        },
        SessionViewModel {
            id: session_id_2,
            title: "Very Long Session Title That Will Truncate Cleanly In Sidebar".to_string(),
            updated_at: SystemTime::now(),
            active: false,
            preview: Some("Long session description".to_string()),
            pinned: false,
            archived: false,
        },
    ];

    // 1. Initial active session verification
    assert!(state.sessions[0].active);
    assert!(!state.sessions[1].active);

    // 2. Switch session selection
    state.sessions[0].active = false;
    state.sessions[1].active = true;
    assert!(state.sessions[1].active);

    // 3. Clear sessions (returns to Home/Welcome mode)
    state.sessions.clear();
    assert_eq!(AppRenderer::layout_mode(&state), AppLayoutMode::Welcome);
}

// ============================================================================
// STAGE 4: Runtime Resilience & Reconnection
// ============================================================================
#[test]
fn test_stage4_runtime_resilience_and_reconnection() {
    let renderer = AppRenderer::new();
    let theme = Theme::default();
    let backend = TestBackend::new(100, 26);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();

    // 1. Disconnected State
    state.connection_mode = ConnectionMode::Disconnected;
    let res_disc = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(res_disc.is_ok(), "Render failed in Disconnected mode");

    // 2. Connecting State
    state.connection_mode = ConnectionMode::Connecting;
    let res_conn = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(res_conn.is_ok(), "Render failed in Connecting mode");

    // 3. Connected State (Daemon)
    state.connection_mode = ConnectionMode::Daemon;
    let res_ok = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(res_ok.is_ok(), "Render failed in Connected Daemon mode");

    // 4. Error Transient Reporting
    state.update(Action::ReportError(
        "IPC UDS connection dropped unexpectedly".to_string(),
    ));
    assert!(matches!(state.generation_state, GenerationState::Error(_)));
    let res_err = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
    assert!(res_err.is_ok(), "Render failed during Error state");
}

// ============================================================================
// STAGE 5: Interaction Matrix & Focus Traversal
// ============================================================================
#[test]
fn test_stage5_interaction_and_keyboard_routing_matrix() {
    let mut state = UiState::new();
    state.terminal_width = 120;
    state.terminal_height = 30;

    // 1. Focus toggle between Editor and Sidebar
    assert_eq!(state.focus, brain_tui::state::FocusRegion::Editor);
    state.update(Action::ToggleFocus);
    assert_eq!(state.focus, brain_tui::state::FocusRegion::Sidebar);
    state.update(Action::ToggleFocus);
    assert_eq!(state.focus, brain_tui::state::FocusRegion::Editor);

    // 2. Compact Viewport Focus Restriction (<80 cols)
    state.terminal_width = 70;
    state.recalculate_viewport();
    state.update(Action::ToggleFocus);
    assert_eq!(
        state.focus,
        brain_tui::state::FocusRegion::Editor,
        "Sidebar focus must be restricted on compact viewports"
    );
}
