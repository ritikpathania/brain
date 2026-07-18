use brain_tui::ui::focus::{FocusManager, FocusProfile};
use brain_tui::ui::interaction::chat::{ChatState, GenerationState};
use brain_tui::ui::interaction::editor::Editor;
use brain_tui::ui::interaction::scroll::ScrollState;
use brain_tui::ui::interaction::sidebar::SidebarInteraction;
use brain_tui::ui::interaction::MessageRole;
use brain_tui::ui::router::{ActiveScreen, ScreenRouter};
use brain_tui::ui::state::AppState;
use brain_tui::ui::widgets::view_models::{ChatScreenView, ConnectionState, FocusTarget};
use brain_tui::ui::widgets::ChatScreen;

const CHAT_VIEW: ChatScreenView<'static> = ChatScreenView {
    session_title: "",
    connection: ConnectionState::Connected,
    is_working: false,
    message_count: 0,
    input_buffer: "",
    focus: FocusTarget::Prompt,
};

#[test]
fn test_offline_tui_graceful_error_handling() {
    let chat = ChatState::new();
    let editor = Editor::new();
    let scroll = ScrollState::new();
    let focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let chat_screen = ChatScreen { view: &CHAT_VIEW };
    let router = ScreenRouter::new(ActiveScreen::Chat(chat_screen));
    let sidebar = SidebarInteraction::new();

    let mut state = AppState::new(chat, editor, scroll, focus, sidebar, router);

    // 1. Submit prompt user message
    let (_user_id, assistant_id) = state.submit_user_message("Test query when offline".to_string());

    // Verify initial generation state is Waiting
    assert!(matches!(state.generation(), &GenerationState::Waiting));

    // 2. Simulate submission failure (offline socket connection)
    let err_msg = "Error: Failed to connect to memory daemon (UDS socket not found)".to_string();
    state.handle_submission_error(assistant_id, err_msg.clone());

    // 3. Verify state transitioned to Error
    match state.generation() {
        &GenerationState::Error { message } => {
            assert_eq!(message, assistant_id);
        }
        other => panic!("Expected GenerationState::Error, got {:?}", other),
    }

    // 4. Verify the chat contains the error warning
    let messages = state.chat().messages();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[1].role, MessageRole::Assistant);

    let doc_str = messages[1].text.raw();
    assert!(doc_str.contains("Error: Failed to connect to memory daemon"));
}
