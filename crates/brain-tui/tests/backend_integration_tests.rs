use brain_tui::ui::focus::{FocusManager, FocusProfile};
use brain_tui::ui::interaction::{ChatState, Editor, GenerationState, MessageId, ScrollState};
use brain_tui::ui::protocol::{BackendCommand, FinishReason, RequestId};
use brain_tui::ui::router::{ActiveScreen, ScreenRouter};
use brain_tui::ui::state::AppState;
use brain_tui::ui::widgets::view_models::{ChatScreenView, ConnectionState, FocusTarget};
use brain_tui::ui::widgets::ChatScreen;

const CHAT_VIEW: ChatScreenView<'static> = ChatScreenView {
    session_title: "test",
    connection: ConnectionState::Connected,
    is_working: false,
    message_count: 0,
    input_buffer: "",
    focus: FocusTarget::Prompt,
};

fn make_test_app_state<'a>() -> AppState<'a> {
    let chat = ChatState::new();
    let editor = Editor::new();
    let scroll = ScrollState::new();
    let focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let chat_screen = ChatScreen { view: &CHAT_VIEW };
    let router = ScreenRouter::new(ActiveScreen::Chat(chat_screen));

    let sidebar = brain_tui::ui::interaction::sidebar::SidebarInteraction::new();
    AppState::new(chat, editor, scroll, focus, sidebar, router)
}

#[test]
fn test_app_state_domain_streaming() {
    let mut state = make_test_app_state();
    assert_eq!(*state.generation(), GenerationState::Idle);

    // 1. Submit prompt text
    let (user_id, assistant_id) = state.submit_user_message("hello".to_string());
    assert_eq!(user_id, MessageId(1));
    assert_eq!(assistant_id, MessageId(2));
    assert_eq!(*state.generation(), GenerationState::Waiting);

    // Verify opaque RequestId and BackendCommand creation
    let req_id = RequestId::new(100);
    let cmd = BackendCommand::SubmitPrompt {
        request: req_id,
        message: assistant_id,
        text: "hello".to_string(),
    };
    assert_eq!(
        cmd,
        BackendCommand::SubmitPrompt {
            request: RequestId::new(100),
            message: assistant_id,
            text: "hello".to_string()
        }
    );

    // 2. Receive first streaming token
    state.append_stream_token(assistant_id, 1, "He").unwrap();
    assert_eq!(
        *state.generation(),
        GenerationState::Streaming {
            message: assistant_id,
            last_sequence: 1,
        }
    );
    assert_eq!(state.chat().messages().len(), 2);
    assert_eq!(state.chat().messages()[1].text.raw(), "He");

    // 3. Receive second streaming token
    state.append_stream_token(assistant_id, 2, "llo").unwrap();
    assert_eq!(
        *state.generation(),
        GenerationState::Streaming {
            message: assistant_id,
            last_sequence: 2,
        }
    );
    assert_eq!(state.chat().messages()[1].text.raw(), "Hello");

    // 4. Finish stream
    state.finish_stream(assistant_id, FinishReason::Completed);
    assert_eq!(
        *state.generation(),
        GenerationState::Completed {
            message: assistant_id
        }
    );

    // 5. Reset back to Idle
    state.reset_generation();
    assert_eq!(*state.generation(), GenerationState::Idle);
}

#[test]
fn test_sequence_monotonicity() {
    let mut state = make_test_app_state();
    let (_, assistant_id) = state.submit_user_message("test".to_string());

    // Send sequence 3
    state.append_stream_token(assistant_id, 3, "world").unwrap();
    assert_eq!(
        *state.generation(),
        GenerationState::Streaming {
            message: assistant_id,
            last_sequence: 3,
        }
    );
    assert_eq!(state.chat().messages()[1].text.raw(), "world");

    // Send sequence 2 (older, out-of-order) -> must be ignored safely under the monotonicity policy
    state
        .append_stream_token(assistant_id, 2, "ignored")
        .unwrap();
    assert_eq!(
        *state.generation(),
        GenerationState::Streaming {
            message: assistant_id,
            last_sequence: 3,
        }
    );
    assert_eq!(state.chat().messages()[1].text.raw(), "world"); // text remains unchanged!

    // Send sequence 3 again (duplicate) -> must be ignored safely
    state.append_stream_token(assistant_id, 3, "dup").unwrap();
    assert_eq!(state.chat().messages()[1].text.raw(), "world");
}

#[test]
fn test_unknown_message_id_and_out_of_order_finished() {
    let mut state = make_test_app_state();
    let (_, assistant_id) = state.submit_user_message("test".to_string());

    // Case 1: token targeting mismatching ID must fail or be ignored
    let wrong_id = MessageId(999);
    let res = state.append_stream_token(wrong_id, 1, "mismatch");
    assert!(
        res.is_err()
            || state
                .chat()
                .messages()
                .iter()
                .all(|m| !m.text.raw().contains("mismatch"))
    );

    // Case 2: Out-of-order Finished event -> Finished arrives first, then delayed Token
    state.finish_stream(assistant_id, FinishReason::Completed);
    assert_eq!(
        *state.generation(),
        GenerationState::Completed {
            message: assistant_id
        }
    );

    // Delayed token arriving after Finished should be ignored safely to prevent corruption
    let res2 = state.append_stream_token(assistant_id, 1, "delayed");
    assert!(res2.is_ok());
    assert_eq!(state.chat().messages()[1].text.raw(), ""); // text remained empty!
}

#[test]
fn test_idempotent_cancellation() {
    let mut state = make_test_app_state();
    let (_, assistant_id) = state.submit_user_message("hello".to_string());

    // 1st cancel request
    state.cancel_stream(assistant_id);
    assert_eq!(
        *state.generation(),
        GenerationState::Cancelling {
            message: assistant_id
        }
    );

    // 2nd cancel request (idempotency check)
    state.cancel_stream(assistant_id);
    assert_eq!(
        *state.generation(),
        GenerationState::Cancelling {
            message: assistant_id
        }
    );

    // Finish event with Cancelled reason arrives
    state.finish_stream(assistant_id, FinishReason::Cancelled);
    assert_eq!(*state.generation(), GenerationState::Idle);

    // Duplicate Finished Cancelled arrives
    state.finish_stream(assistant_id, FinishReason::Cancelled);
    assert_eq!(*state.generation(), GenerationState::Idle);
}
