use brain_tui::ui::focus::{FocusManager, FocusProfile, FocusScope};
use brain_tui::ui::input::{Command, InputAction, InputRouter, TextInput};
use brain_tui::ui::widgets::view_models::FocusTarget;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn test_focus_manager_cyclic_traversal() {
    let mut fm = FocusManager::new(FocusTarget::Sidebar, FocusProfile::Chat);

    assert_eq!(fm.current(), FocusTarget::Sidebar);
    assert_eq!(fm.scope(), FocusScope::Screen);

    // Tab moves focus forward: Sidebar -> Conversation -> Prompt -> Sidebar
    fm.next();
    assert_eq!(fm.current(), FocusTarget::Conversation);

    fm.next();
    assert_eq!(fm.current(), FocusTarget::Prompt);

    fm.next();
    assert_eq!(fm.current(), FocusTarget::Sidebar);
}

#[test]
fn test_focus_manager_traversal_inversion() {
    let mut fm = FocusManager::new(FocusTarget::Sidebar, FocusProfile::Chat);

    // next then prev should be identity
    fm.next();
    fm.prev();
    assert_eq!(fm.current(), FocusTarget::Sidebar);

    // prev moves focus backward: Sidebar -> Prompt -> Conversation -> Sidebar
    fm.prev();
    assert_eq!(fm.current(), FocusTarget::Prompt);

    fm.prev();
    assert_eq!(fm.current(), FocusTarget::Conversation);
}

#[test]
fn test_input_router_referential_transparency() {
    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::empty());
    let action1 = InputRouter::handle(key);
    let action2 = InputRouter::handle(key);

    assert_eq!(action1, action2);
}

#[test]
fn test_input_router_mappings() {
    // Tab -> FocusNext
    let key_tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::empty());
    assert_eq!(
        InputRouter::handle(key_tab),
        InputAction::Command(Command::FocusNext)
    );

    // Shift+Tab -> BackTab -> FocusPrevious
    let key_backtab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty());
    assert_eq!(
        InputRouter::handle(key_backtab),
        InputAction::Command(Command::FocusPrevious)
    );

    // Ctrl+C -> Exit
    let key_ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        InputRouter::handle(key_ctrl_c),
        InputAction::Command(Command::Exit)
    );

    // Char input -> Text Char
    let key_char = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
    assert_eq!(
        InputRouter::handle(key_char),
        InputAction::Text(TextInput::Char('a'))
    );
}

#[test]
fn test_phase1_home_to_workspace_and_back_navigation() {
    use brain_tui::state::{Action, UiState};
    use brain_tui::ui::navigation::Screen;

    let mut state = UiState::new();
    assert_eq!(state.screen, Screen::Home);

    // Home + Left -> Workspace
    state.update(Action::NavigateToWorkspace);
    assert_eq!(state.screen, Screen::Workspace);

    // Workspace + Right -> Home
    state.update(Action::NavigateToHome);
    assert_eq!(state.screen, Screen::Home);
}

#[test]
fn test_phase1_workspace_selection_up_down() {
    use brain_domain::SessionId;
    use brain_tui::state::{Action, SessionViewModel, UiState};
    use std::time::SystemTime;

    let mut state = UiState::new();
    state.sessions = vec![
        SessionViewModel {
            id: SessionId::new(),
            title: "Session 1".to_string(),
            updated_at: SystemTime::now(),
            active: false,
            preview: None,
            pinned: false,
            archived: false,
        },
        SessionViewModel {
            id: SessionId::new(),
            title: "Session 2".to_string(),
            updated_at: SystemTime::now(),
            active: false,
            preview: None,
            pinned: false,
            archived: false,
        },
    ];
    state.selected_session_idx = 0;

    // Down moves to index 1
    state.update(Action::SelectNextSession);
    assert_eq!(state.selected_session_idx, 1);

    // Down at boundary stays at 1
    state.update(Action::SelectNextSession);
    assert_eq!(state.selected_session_idx, 1);

    // Up moves back to index 0
    state.update(Action::SelectPreviousSession);
    assert_eq!(state.selected_session_idx, 0);

    // Up at top stays at index 0
    state.update(Action::SelectPreviousSession);
    assert_eq!(state.selected_session_idx, 0);
}

#[test]
fn test_phase1_workspace_enter_opens_session() {
    use brain_domain::SessionId;
    use brain_tui::state::{Action, FocusRegion, SessionViewModel, UiState};
    use brain_tui::ui::navigation::Screen;
    use std::time::SystemTime;

    let mut state = UiState::new();
    let id_target = SessionId::new();
    state.sessions = vec![SessionViewModel {
        id: id_target,
        title: "Session 1".to_string(),
        updated_at: SystemTime::now(),
        active: false,
        preview: None,
        pinned: false,
        archived: false,
    }];
    state.selected_session_idx = 0;

    state.update(Action::OpenSelectedSession);
    assert_eq!(state.screen, Screen::Workspace);
    assert_eq!(state.focus, FocusRegion::Editor);
    assert_eq!(state.session_id, id_target);
}

#[test]
fn test_phase1_workspace_space_and_ctrl_x_modals() {
    use brain_domain::SessionId;
    use brain_tui::state::{Action, SessionViewModel, UiState};
    use brain_tui::ui::navigation::Modal;
    use std::time::SystemTime;

    let mut state = UiState::new();
    state.sessions = vec![SessionViewModel {
        id: SessionId::new(),
        title: "Session 1".to_string(),
        updated_at: SystemTime::now(),
        active: false,
        preview: None,
        pinned: false,
        archived: false,
    }];

    // Space -> ReplyComposer modal
    state.update(Action::OpenReplyComposer);
    assert_eq!(state.modal, Some(Modal::ReplyComposer));

    // Esc -> CloseModal
    state.update(Action::CloseModal);
    assert_eq!(state.modal, None);

    // Ctrl+X -> ConfirmDelete modal
    state.update(Action::OpenDeleteConfirmation);
    assert_eq!(state.modal, Some(Modal::ConfirmDelete));

    // Esc -> CloseModal
    state.update(Action::CloseModal);
    assert_eq!(state.modal, None);
}

#[test]
fn test_phase1_delete_confirmation_execution() {
    use brain_domain::SessionId;
    use brain_tui::state::{Action, SessionViewModel, UiState};
    use brain_tui::ui::navigation::Modal;
    use std::time::SystemTime;

    let mut state = UiState::new();
    state.sessions = vec![SessionViewModel {
        id: SessionId::new(),
        title: "Session 1".to_string(),
        updated_at: SystemTime::now(),
        active: false,
        preview: None,
        pinned: false,
        archived: false,
    }];

    state.update(Action::OpenDeleteConfirmation);
    assert_eq!(state.modal, Some(Modal::ConfirmDelete));

    state.update(Action::ConfirmDeleteSession);
    assert_eq!(state.modal, None);
    assert!(state.sessions.is_empty());
}

#[test]
fn test_phase1_empty_workspace_selection_no_panic() {
    use brain_tui::state::{Action, UiState};

    let mut state = UiState::new();
    state.sessions = vec![];
    state.selected_session_idx = 0;

    // Must not panic on empty sessions
    state.update(Action::SelectNextSession);
    state.update(Action::SelectPreviousSession);
    state.update(Action::OpenSelectedSession);
    state.update(Action::OpenReplyComposer);
    state.update(Action::OpenDeleteConfirmation);
    state.update(Action::ConfirmDeleteSession);

    assert_eq!(state.modal, None);
}

#[test]
fn test_phase1_modal_input_isolation() {
    use brain_tui::state::{Action, UiState};
    use brain_tui::ui::navigation::Modal;

    let mut state = UiState::new();
    state.modal = Some(Modal::ConfirmDelete);

    // When modal is open, CloseModal closes it cleanly
    state.update(Action::CloseModal);
    assert_eq!(state.modal, None);
}

#[test]
fn test_phase2_first_home_query_creates_exactly_one_session() {
    use brain_tui::state::{Action, UiState, UpdateResult};
    use brain_tui::ui::navigation::Screen;

    let mut state = UiState::new();
    assert_eq!(state.screen, Screen::Home);
    assert!(state.sessions.is_empty());

    // Type query into editor on Home
    for c in "What is Rust?".chars() {
        state.editor.insert(c);
    }
    let res = state.update(Action::SubmitPrompt);

    assert_eq!(
        res,
        UpdateResult::PromptSubmitted("What is Rust?".to_string())
    );
    assert_eq!(state.sessions.len(), 1);
    assert_eq!(state.sessions[0].id, state.session_id);
}

#[test]
fn test_phase2_query_persisted_in_active_messages_and_session_history() {
    use brain_tui::state::{Action, UiState};

    let mut state = UiState::new();
    for c in "Tell me about memory safety.".chars() {
        state.editor.insert(c);
    }
    state.update(Action::SubmitPrompt);

    assert_eq!(state.active_messages.len(), 1);
    assert_eq!(
        state.active_messages[0].content,
        "Tell me about memory safety."
    );
    assert_eq!(
        state
            .session_histories
            .get(&state.session_id)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn test_phase2_answer_appears_in_home_conversation_state() {
    use brain_tui::state::{Action, RenderToken, UiState};

    let mut state = UiState::new();
    for c in "Explain ownership.".chars() {
        state.editor.insert(c);
    }
    state.update(Action::SubmitPrompt);

    // Simulate typewriter receiving answer tokens & response completion
    state.update(Action::ReceiveToken(RenderToken::Text(
        "Ownership is...".to_string(),
    )));
    state.active_response = "Ownership is...".to_string();
    state.update(Action::FinishStream);
    state.commit_active_response();

    assert_eq!(state.active_messages.len(), 2);
    assert_eq!(state.active_messages[1].content, "Ownership is...");
}

#[test]
fn test_phase2_second_query_reuses_same_session() {
    use brain_tui::state::{Action, UiState};

    let mut state = UiState::new();
    for c in "First question".chars() {
        state.editor.insert(c);
    }
    state.update(Action::SubmitPrompt);

    let first_session_id = state.session_id;
    assert_eq!(state.sessions.len(), 1);

    for c in "Second question".chars() {
        state.editor.insert(c);
    }
    state.update(Action::SubmitPrompt);

    // Must reuse the exact same session_id and sessions vector length
    assert_eq!(state.session_id, first_session_id);
    assert_eq!(state.sessions.len(), 1);
    assert_eq!(state.active_messages.len(), 2);
}

#[test]
fn test_phase2_navigating_home_workspace_home_preserves_conversation() {
    use brain_tui::state::{Action, UiState};
    use brain_tui::ui::navigation::Screen;

    let mut state = UiState::new();
    for c in "Preserved prompt".chars() {
        state.editor.insert(c);
    }
    state.update(Action::SubmitPrompt);
    let original_session_id = state.session_id;

    // Home -> Workspace
    state.update(Action::NavigateToWorkspace);
    assert_eq!(state.screen, Screen::Workspace);

    // Workspace -> Home
    state.update(Action::NavigateToHome);
    assert_eq!(state.screen, Screen::Home);

    // Active messages and session ID must be preserved
    assert_eq!(state.session_id, original_session_id);
    assert_eq!(state.active_messages.len(), 1);
    assert_eq!(state.sessions.len(), 1);
}

#[test]
fn test_phase2_empty_prompt_does_not_create_session() {
    use brain_tui::state::{Action, UiState, UpdateResult};

    let mut state = UiState::new();
    for c in "   ".chars() {
        state.editor.insert(c);
    }
    let res = state.update(Action::SubmitPrompt);

    assert_eq!(res, UpdateResult::NoChange);
    assert!(state.sessions.is_empty());
    assert!(state.active_messages.is_empty());
}

#[test]
fn test_phase2_failed_query_does_not_manufacture_answer() {
    use brain_tui::state::{Action, UiState};

    let mut state = UiState::new();
    for c in "Failed query test".chars() {
        state.editor.insert(c);
    }
    state.update(Action::SubmitPrompt);

    // Report error stream failure
    state.update(Action::ReportError("Network timeout".to_string()));

    // User message is present, but no manufactured assistant answer
    assert_eq!(state.active_messages.len(), 1);
    assert_eq!(
        state.active_messages[0].role,
        brain_domain::MessageRole::User
    );
}

#[test]
fn test_phase2_no_duplicate_session_created_on_rerender() {
    use brain_tui::state::{Action, UiState};

    let mut state = UiState::new();
    for c in "Single session query".chars() {
        state.editor.insert(c);
    }
    state.update(Action::SubmitPrompt);

    // Multiple state ticks / navigation actions
    state.update(Action::NavigateToWorkspace);
    state.update(Action::NavigateToHome);
    state.update(Action::NavigateToWorkspace);
    state.update(Action::NavigateToHome);

    assert_eq!(state.sessions.len(), 1);
}

#[test]
fn test_phase3_real_backend_session_summaries_populate_workspace() {
    use brain_domain::SessionId;
    use brain_tui::client::SessionSummary;
    use brain_tui::state::{Action, UiState};
    use std::time::SystemTime;

    let mut state = UiState::new();
    let s1 = SessionId::new();
    let s2 = SessionId::new();

    let summaries = vec![
        SessionSummary {
            id: s1,
            title: "Backend Session 1".to_string(),
            updated_at: SystemTime::now(),
            pinned: true,
            archived: false,
        },
        SessionSummary {
            id: s2,
            title: "Backend Session 2".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
    ];

    state.update(Action::LoadSessions(summaries));

    assert_eq!(state.sessions.len(), 2);
    assert_eq!(state.sessions[0].id, s1);
    assert_eq!(state.sessions[0].title, "Backend Session 1");
    assert!(state.sessions[0].pinned);
    assert_eq!(state.sessions[1].id, s2);
}

#[test]
fn test_phase3_empty_workspace_is_safe() {
    use brain_tui::state::{Action, UiState};

    let mut state = UiState::new();
    state.update(Action::LoadSessions(vec![]));

    assert!(state.sessions.is_empty());
    state.update(Action::SelectNextSession);
    state.update(Action::SelectPreviousSession);
    state.update(Action::OpenSelectedSession);
    state.update(Action::OpenDeleteConfirmation);
    state.update(Action::ConfirmDeleteSession);

    assert_eq!(state.selected_session_idx, 0);
}

#[test]
fn test_phase3_up_down_selection_navigation_and_boundaries() {
    use brain_domain::SessionId;
    use brain_tui::client::SessionSummary;
    use brain_tui::state::{Action, UiState};
    use std::time::SystemTime;

    let mut state = UiState::new();
    state.update(Action::LoadSessions(vec![
        SessionSummary {
            id: SessionId::new(),
            title: "Session A".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
        SessionSummary {
            id: SessionId::new(),
            title: "Session B".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
    ]));
    state.selected_session_idx = 0;

    // Down moves to index 1
    state.update(Action::SelectNextSession);
    assert_eq!(state.selected_session_idx, 1);

    // Down at bottom stays at 1
    state.update(Action::SelectNextSession);
    assert_eq!(state.selected_session_idx, 1);

    // Up moves to index 0
    state.update(Action::SelectPreviousSession);
    assert_eq!(state.selected_session_idx, 0);

    // Up at top stays at index 0
    state.update(Action::SelectPreviousSession);
    assert_eq!(state.selected_session_idx, 0);
}

#[test]
fn test_phase3_enter_opens_selected_persisted_session() {
    use brain_domain::SessionId;
    use brain_tui::client::SessionSummary;
    use brain_tui::state::{Action, FocusRegion, UiState};
    use brain_tui::ui::navigation::Screen;
    use std::time::SystemTime;

    let mut state = UiState::new();
    let target_id = SessionId::new();
    state.update(Action::LoadSessions(vec![SessionSummary {
        id: target_id,
        title: "Target Session".to_string(),
        updated_at: SystemTime::now(),
        pinned: false,
        archived: false,
    }]));
    state.selected_session_idx = 0;

    state.update(Action::OpenSelectedSession);

    assert_eq!(state.session_id, target_id);
    assert_eq!(state.screen, Screen::Workspace);
    assert_eq!(state.focus, FocusRegion::Editor);
}

#[test]
fn test_phase3_opened_session_renders_persisted_conversation_history() {
    use brain_domain::{Message, MessageId, MessageRole, SessionId};
    use brain_tui::client::SessionSummary;
    use brain_tui::state::{Action, UiState};
    use std::time::SystemTime;

    let mut state = UiState::new();
    let s_id = SessionId::new();
    state.update(Action::LoadSessions(vec![SessionSummary {
        id: s_id,
        title: "Session with History".to_string(),
        updated_at: SystemTime::now(),
        pinned: false,
        archived: false,
    }]));

    // Pre-populate session history (e.g. from backend load_session)
    let history_msg = Message::new(
        MessageId::new(),
        MessageRole::User,
        "Persisted message".to_string(),
    );
    state
        .session_histories
        .insert(s_id, vec![history_msg.clone()]);
    state.selected_session_idx = 0;

    state.update(Action::OpenSelectedSession);

    assert_eq!(state.active_messages.len(), 1);
    assert_eq!(state.active_messages[0].content, "Persisted message");
}

#[test]
fn test_phase3_space_opens_reply_composer_and_esc_cancels() {
    use brain_domain::SessionId;
    use brain_tui::client::SessionSummary;
    use brain_tui::state::{Action, UiState};
    use brain_tui::ui::navigation::Modal;
    use std::time::SystemTime;

    let mut state = UiState::new();
    state.update(Action::LoadSessions(vec![SessionSummary {
        id: SessionId::new(),
        title: "Reply Target".to_string(),
        updated_at: SystemTime::now(),
        pinned: false,
        archived: false,
    }]));

    // Space -> ReplyComposer
    state.update(Action::OpenReplyComposer);
    assert_eq!(state.modal, Some(Modal::ReplyComposer));

    // Esc -> CloseModal
    state.update(Action::CloseModal);
    assert_eq!(state.modal, None);
}

#[test]
fn test_phase3_enter_in_reply_composer_submits_to_selected_session() {
    use brain_domain::SessionId;
    use brain_tui::client::SessionSummary;
    use brain_tui::state::{Action, UiState, UpdateResult};
    use std::time::SystemTime;

    let mut state = UiState::new();
    let target_id = SessionId::new();
    state.update(Action::LoadSessions(vec![SessionSummary {
        id: target_id,
        title: "Reply Target".to_string(),
        updated_at: SystemTime::now(),
        pinned: false,
        archived: false,
    }]));
    state.session_id = target_id;

    for c in "Replying to session".chars() {
        state.editor.insert(c);
    }
    let res = state.update(Action::SubmitPrompt);

    assert_eq!(
        res,
        UpdateResult::PromptSubmitted("Replying to session".to_string())
    );
    assert_eq!(state.session_id, target_id);
    assert_eq!(state.active_messages[0].content, "Replying to session");
}

#[test]
fn test_phase3_ctrl_x_delete_flow_and_reconciliation() {
    use brain_domain::SessionId;
    use brain_tui::client::SessionSummary;
    use brain_tui::state::{Action, UiState};
    use brain_tui::ui::navigation::Modal;
    use std::time::SystemTime;

    let mut state = UiState::new();
    let s1 = SessionId::new();
    let s2 = SessionId::new();

    state.update(Action::LoadSessions(vec![
        SessionSummary {
            id: s1,
            title: "Delete Me".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
        SessionSummary {
            id: s2,
            title: "Keep Me".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
    ]));
    state.selected_session_idx = 0;

    // Ctrl+X -> OpenDeleteConfirmation
    state.update(Action::OpenDeleteConfirmation);
    assert_eq!(state.modal, Some(Modal::ConfirmDelete));

    // Esc cancels deletion
    state.update(Action::CloseModal);
    assert_eq!(state.modal, None);
    assert_eq!(state.sessions.len(), 2);

    // Ctrl+X then Enter confirms deletion
    state.update(Action::OpenDeleteConfirmation);
    state.update(Action::ConfirmDeleteSession);
    assert_eq!(state.modal, None);
    assert_eq!(state.sessions.len(), 1);
    assert_eq!(state.sessions[0].id, s2);
    assert_eq!(state.selected_session_idx, 0);
}

#[test]
fn test_phase3_right_arrow_returns_to_home_preserving_session() {
    use brain_tui::state::{Action, UiState};
    use brain_tui::ui::navigation::Screen;

    let mut state = UiState::new();
    state.screen = Screen::Workspace;
    for c in "Session query".chars() {
        state.editor.insert(c);
    }
    state.update(Action::SubmitPrompt);
    let active_id = state.session_id;

    // Workspace + Right -> Home
    state.update(Action::NavigateToHome);
    assert_eq!(state.screen, Screen::Home);
    assert_eq!(state.session_id, active_id);
    assert_eq!(state.active_messages.len(), 1);
}

#[test]
fn test_verification_reply_composer_target_identity_differs_from_current_session() {
    use brain_domain::SessionId;
    use brain_tui::client::{ExecutionOptions, ExecutionRequest, SessionSummary};
    use brain_tui::state::{Action, UiState, UpdateResult};
    use brain_tui::ui::navigation::Modal;
    use std::time::SystemTime;
    use tokio_util::sync::CancellationToken;

    let mut state = UiState::new();
    let session_a = SessionId::new();
    let session_b = SessionId::new();

    state.update(Action::LoadSessions(vec![
        SessionSummary {
            id: session_a,
            title: "Session A".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
        SessionSummary {
            id: session_b,
            title: "Session B".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
    ]));

    // Current session is Session A
    state.session_id = session_a;
    // Selection points to Session B (index 1)
    state.selected_session_idx = 1;

    // Open Reply Composer
    state.update(Action::OpenReplyComposer);
    assert_eq!(state.modal, Some(Modal::ReplyComposer));

    // MUST update state.session_id to Session B
    assert_eq!(state.session_id, session_b);

    // Type prompt and submit
    for c in "Replying to B".chars() {
        state.editor.insert(c);
    }
    let res = state.update(Action::SubmitPrompt);

    // Must submit prompt
    assert_eq!(
        res,
        UpdateResult::PromptSubmitted("Replying to B".to_string())
    );

    // Construct ExecutionRequest from state and verify it carries Session B's ID
    let req = ExecutionRequest {
        session_id: state.session_id,
        prompt: "Replying to B".to_string(),
        options: ExecutionOptions::default(),
        cancellation_token: CancellationToken::new(),
        workspace_context: None,
    };

    assert_eq!(req.session_id, session_b);
    assert_ne!(req.session_id, session_a);
}

#[test]
fn test_verification_delete_failure_semantics_preserves_uistate_sessions() {
    use brain_domain::SessionId;
    use brain_tui::client::SessionSummary;
    use brain_tui::state::{Action, UiState};
    use brain_tui::ui::navigation::Modal;
    use std::time::SystemTime;

    let mut state = UiState::new();
    let session_a = SessionId::new();
    let session_b = SessionId::new();

    state.update(Action::LoadSessions(vec![
        SessionSummary {
            id: session_a,
            title: "Session A".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
        SessionSummary {
            id: session_b,
            title: "Session B".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
    ]));

    state.selected_session_idx = 0;
    state.update(Action::OpenDeleteConfirmation);
    assert_eq!(state.modal, Some(Modal::ConfirmDelete));

    // On backend deletion failure, event loop dispatches ReportError and CloseModal
    state.update(Action::ReportError(
        "Failed to delete session: Database locked".to_string(),
    ));
    state.update(Action::CloseModal);

    // Sessions list MUST be preserved (Session A was NOT deleted)
    assert_eq!(state.sessions.len(), 2);
    assert_eq!(state.sessions[0].id, session_a);
    assert_eq!(state.sessions[1].id, session_b);
    assert_eq!(state.modal, None);
}

#[test]
fn test_verification_delete_success_semantics_removes_session() {
    use brain_domain::SessionId;
    use brain_tui::client::SessionSummary;
    use brain_tui::state::{Action, UiState};
    use std::time::SystemTime;

    let mut state = UiState::new();
    let session_a = SessionId::new();
    let session_b = SessionId::new();

    state.update(Action::LoadSessions(vec![
        SessionSummary {
            id: session_a,
            title: "Session A".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
        SessionSummary {
            id: session_b,
            title: "Session B".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        },
    ]));

    state.selected_session_idx = 0;
    state.update(Action::OpenDeleteConfirmation);

    // On backend deletion success, event loop dispatches DeleteSession and CloseModal
    state.update(Action::DeleteSession(session_a));
    state.update(Action::CloseModal);

    // Session A is removed, Session B remains
    assert_eq!(state.sessions.len(), 1);
    assert_eq!(state.sessions[0].id, session_b);
    assert_eq!(state.modal, None);
}
