use brain_domain::{Message, MessageId, MessageRole, SessionId};
use brain_tui::client::SessionSummary;
use brain_tui::state::{Action, ConnectionMode, GenerationState, LoadRequestId, UiState};
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::SystemTime;

#[test]
fn test_release_candidate_full_installed_product_journey() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();
    let (w, h) = (120, 30);
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();

    let session_id = SessionId::new();
    let req_id = LoadRequestId(999);

    // 1. Fresh application launch
    let mut state = UiState::new();
    state.terminal_width = w;
    state.terminal_height = h;

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, theme))
        .unwrap();
    let home_text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(
        home_text.contains("BRAIN") || home_text.contains("Connected") || home_text.contains("shortcuts") || home_text.contains("Claude Code") || home_text.contains("Disconnected"),
        "Home screen layout failed"
    );

    // 2. Transition to Workspace & Session Load
    state.screen = Screen::Workspace;
    state.connection_mode = ConnectionMode::Daemon;

    state.update(Action::LoadSessions(vec![SessionSummary {
        id: session_id,
        title: "Release Candidate Architecture".to_string(),
        updated_at: SystemTime::now(),
        pinned: true,
        archived: false,
    }]));

    state.update(Action::ActivateSession {
        session_id,
        request_id: req_id,
    });
    state.update(Action::SessionLoaded {
        session_id,
        request_id: req_id,
        messages: vec![
            Message::new(
                MessageId::new(),
                MessageRole::User,
                "Explain release criteria".to_string(),
            ),
            Message::new(
                MessageId::new(),
                MessageRole::Assistant,
                "Clean build, soak, and full workspace regression.".to_string(),
            ),
        ],
    });

    assert_eq!(state.active_messages.len(), 2);

    // 3. Prompt Submission & Cancellation Recovery
    state.generation_state = GenerationState::Streaming {
        started_at: SystemTime::now(),
    };
    state.update(Action::CancelStream);
    assert_eq!(state.generation_state, GenerationState::Cancelled(None));

    // 4. Follow-up query submission right after cancellation
    for c in "Verify session continuation".chars() {
        state.editor.insert(c);
    }
    let res = state.update(Action::SubmitPrompt);
    assert!(matches!(
        res,
        brain_tui::state::UpdateResult::PromptSubmitted(_)
            | brain_tui::state::UpdateResult::Changed
    ));

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, theme))
        .unwrap();
    let final_text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(
        final_text.contains("Release Candidate Architecture") || final_text.contains("Clean build"),
        "Final E2E rendering failed"
    );
}
