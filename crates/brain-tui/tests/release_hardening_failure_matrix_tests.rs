use brain_domain::SessionId;
use brain_tui::state::{Action, GenerationState, LoadRequestId, UiState};
use brain_tui::ui::navigation::Screen;

#[test]
fn test_release_hardening_cancellation_followed_by_immediate_new_query() {
    let mut state = UiState::new();
    state.screen = Screen::Workspace;
    state.generation_state = GenerationState::Streaming {
        started_at: std::time::SystemTime::now(),
    };
    state.active_response = "Partial response text before cancellation...".to_string();

    // 1. User presses Esc -> Action::CancelStream
    state.update(Action::CancelStream);
    assert_eq!(state.generation_state, GenerationState::Cancelled(None));
    assert_eq!(
        state.transient_message.as_ref().map(|(m, _)| m.as_str()),
        Some("Request cancelled")
    );

    // 2. Immediate query submission right after cancellation (Cancellation -> Immediate Query)
    for c in "What is the graph schema?".chars() {
        state.editor.insert(c);
    }
    let update_res = state.update(Action::SubmitPrompt);
    assert!(matches!(
        update_res,
        brain_tui::state::UpdateResult::PromptSubmitted(_)
            | brain_tui::state::UpdateResult::Changed
    ));
    assert_eq!(state.editor.text(), "");
}

#[test]
fn test_release_hardening_stale_stream_and_load_rejection_taxonomy() {
    let mut state = UiState::new();
    let session_active = SessionId::new();
    let session_stale = SessionId::new();

    let req_active = LoadRequestId(500);
    let req_stale = LoadRequestId(499);

    // Set pending load for session_active
    state.update(Action::ActivateSession {
        session_id: session_active,
        request_id: req_active,
    });

    // Stale session load payload arrives -> EXPECTED_RECOVERY / REJECTED
    let stale_res = state.update(Action::SessionLoaded {
        session_id: session_stale,
        request_id: req_stale,
        messages: vec![],
    });

    assert_eq!(stale_res, brain_tui::state::UpdateResult::NoChange);
    assert_ne!(state.session_id, session_stale);

    // Active matching payload arrives -> APPLIED
    let active_res = state.update(Action::SessionLoaded {
        session_id: session_active,
        request_id: req_active,
        messages: vec![],
    });

    assert_eq!(active_res, brain_tui::state::UpdateResult::Changed);
    assert_eq!(state.session_id, session_active);
}
