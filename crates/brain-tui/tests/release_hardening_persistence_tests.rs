use brain_domain::{Message, MessageId, MessageRole, SessionId};
use brain_tui::client::SessionSummary;
use brain_tui::state::{Action, ConnectionMode, LoadRequestId, UiState};
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::SystemTime;

#[test]
fn test_release_hardening_persistence_across_restart_flow() {
    let session_id = SessionId::new();
    let req_1 = LoadRequestId(100);
    let req_2 = LoadRequestId(101);

    // --- PROCESS A: Create session, submit prompt, receive answer, save ---
    let mut state_a = UiState::new();
    state_a.screen = Screen::Workspace;
    state_a.connection_mode = ConnectionMode::Daemon;

    state_a.update(Action::LoadSessions(vec![SessionSummary {
        id: session_id,
        title: "Persisted Architecture Thread".to_string(),
        updated_at: SystemTime::now(),
        pinned: false,
        archived: false,
    }]));
    state_a.selected_session_idx = 0;

    let initial_messages = vec![
        Message::new(
            MessageId::new(),
            MessageRole::User,
            "What is the storage engine?".to_string(),
        ),
        Message::new(
            MessageId::new(),
            MessageRole::Assistant,
            "SQLite in brain-storage is the sole session authority.".to_string(),
        ),
    ];

    state_a.update(Action::ActivateSession {
        session_id,
        request_id: req_1,
    });
    state_a.update(Action::SessionLoaded {
        session_id,
        request_id: req_1,
        messages: initial_messages.clone(),
    });

    assert_eq!(state_a.active_messages.len(), 2);
    assert_eq!(state_a.session_id, session_id);

    // --- SIMULATED PROCESS RESTART: Fresh UiState B, loading same session_id ---
    let mut state_b = UiState::new();
    state_b.screen = Screen::Workspace;
    state_b.connection_mode = ConnectionMode::Daemon;

    // 1. Session summary appears in Workspace
    state_b.update(Action::LoadSessions(vec![SessionSummary {
        id: session_id,
        title: "Persisted Architecture Thread".to_string(),
        updated_at: SystemTime::now(),
        pinned: false,
        archived: false,
    }]));
    state_b.selected_session_idx = 0;

    // 2. Open Session -> Pending request
    state_b.update(Action::ActivateSession {
        session_id,
        request_id: req_2,
    });
    assert!(state_b.pending_load.is_some());

    // 3. SQLite load returns identical historical messages
    state_b.update(Action::SessionLoaded {
        session_id,
        request_id: req_2,
        messages: initial_messages.clone(),
    });

    assert_eq!(state_b.session_id, session_id);
    assert_eq!(state_b.active_messages.len(), 2);
    assert_eq!(
        state_b.active_messages[1].content,
        "SQLite in brain-storage is the sole session authority."
    );

    // 4. Submit follow-up prompt to verify session continuation with same SessionId
    for c in "And how are events logged?".chars() {
        state_b.editor.insert(c);
    }
    state_b.update(Action::SubmitPrompt);
    assert_eq!(state_b.session_id, session_id);

    let renderer = AppRenderer::new();
    let theme = dark_theme();
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state_b, theme))
        .unwrap();

    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(
        text.contains("Persisted Architecture") || text.contains("SQLite"),
        "Missing persisted session in renderer viewport"
    );
}
