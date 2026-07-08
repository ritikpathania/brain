use brain_tui::ui::interaction::{
    Editor, ScrollState, Dispatcher, InteractionContext, DispatchResult,
    ChatState, MessageRole, UiEvent, MessageId, AutoFollowPolicy,
    SidebarInteraction, SessionLookup
};
use brain_tui::ui::command::completion::SlashCompletionState;
use brain_tui::ui::command::palette::CommandPaletteState;


use brain_tui::ui::focus::{FocusManager, FocusProfile};
use brain_tui::ui::widgets::view_models::FocusTarget;
use brain_tui::ui::input::{InputAction, Command};
use brain_domain::SessionId;

struct DummyLookup;
impl SessionLookup for DummyLookup {
    fn title(&self, _id: SessionId) -> Option<&str> { None }
}

#[test]
fn test_chat_state_message_ids_ordering() {
    let mut chat = ChatState::new();
    let id1 = chat.push_message(MessageRole::User, "Hello".to_string());
    let id2 = chat.push_message(MessageRole::Assistant, "Hi".to_string());

    assert!(id2 > id1);
    assert_eq!(id1, MessageId(1));
    assert_eq!(id2, MessageId(2));

    // Clear history, next ID must keep increasing
    chat.clear();
    assert_eq!(chat.messages().len(), 0);

    let id3 = chat.push_message(MessageRole::User, "New".to_string());
    assert_eq!(id3, MessageId(3));
}

#[test]
fn test_transactional_submission() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);

    editor.insert('h');
    editor.insert('i');

    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let visible_ids = vec![];
    let lookup = DummyLookup;

    let mut pending_approvals = vec![];

    let res = Dispatcher::dispatch(
        InputAction::Command(Command::Submit),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals, sessions: &[], active_messages: &[],
        }
    );



    // Transactional Submit event returned
    assert_eq!(
        res,
        DispatchResult {
            needs_render: true,
            should_exit: false,
            ui_event: Some(UiEvent::SubmitPrompt("hi".to_string())),
        }
    );

    // Prompt is NOT cleared inside the dispatcher itself (transactional safety)
    assert_eq!(editor.text(), "hi");

    // Application choice: If transaction is accepted, application pushes to ChatState and resets editor
    if let Some(UiEvent::SubmitPrompt(text)) = res.ui_event {
        let mut chat = ChatState::new();
        chat.push_message(MessageRole::User, text);
        editor = Editor::new(); // transaction committed
    }

    assert_eq!(editor.text(), "");
}

#[test]
fn test_empty_whitespace_submissions() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);

    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let visible_ids = vec![];
    let lookup = DummyLookup;

    let mut pending_approvals = vec![];

    // Case 1: Empty editor
    let res = Dispatcher::dispatch(
        InputAction::Command(Command::Submit),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals, sessions: &[], active_messages: &[],
        }
    );


    assert_eq!(res.ui_event, None);

    // Case 2: Whitespace only
    editor.insert(' ');
    editor.insert(' ');
    let res = Dispatcher::dispatch(
        InputAction::Command(Command::Submit),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals, sessions: &[], active_messages: &[],
        }
    );


    assert_eq!(res.ui_event, None);
    assert_eq!(editor.text(), "  "); // editor buffer preserved
}

#[test]
fn test_scroll_pinning_and_resumption() {
    let mut scroll = ScrollState::new();
    
    // Initial state: pinned to bottom
    assert_eq!(scroll.policy, AutoFollowPolicy::Pinned);

    scroll.update_bounds(10, 4); // max_offset = 6
    assert_eq!(scroll.offset(), 6);

    // Adding message keeps scroll pinned to bottom
    scroll.update_bounds(12, 4); // max_offset = 8
    assert_eq!(scroll.offset(), 8);

    // Scrolling up unpins from bottom
    scroll.scroll_up();
    assert_eq!(scroll.offset(), 7);
    assert_eq!(scroll.policy, AutoFollowPolicy::Manual);

    // Pushing new message does NOT force scroll changes while unpinned
    scroll.update_bounds(15, 4); // max_offset = 11
    assert_eq!(scroll.offset(), 7); // preserved

    // Scrolling back down to bottom re-pins auto-scroll
    scroll.scroll_down(); // offset = 8
    scroll.scroll_down(); // offset = 9
    scroll.scroll_down(); // offset = 10
    scroll.scroll_down(); // offset = 11 (max)
    assert_eq!(scroll.policy, AutoFollowPolicy::Pinned);

    // New additions follow bottom again
    scroll.update_bounds(16, 4); // max_offset = 12
    assert_eq!(scroll.offset(), 12);
}
