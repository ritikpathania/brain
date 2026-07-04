use brain_tui::ui::interaction::{
    Editor, ScrollState, Dispatcher, InteractionContext, DispatchResult,
    SidebarInteraction, SessionLookup
};
use brain_tui::ui::focus::{FocusManager, FocusProfile};
use brain_tui::ui::widgets::view_models::FocusTarget;
use brain_tui::ui::input::{InputAction, Command, TextInput};
use brain_domain::SessionId;

struct DummyLookup;
impl SessionLookup for DummyLookup {
    fn title(&self, _id: SessionId) -> Option<&str> { None }
}

#[test]
fn test_editor_cursor_reversibility() {
    let mut editor = Editor::new();
    editor.insert('a');
    editor.insert('b');
    editor.insert('c');
    
    assert_eq!(editor.text(), "abc");
    assert_eq!(editor.cursor().byte_index, 3);
    assert_eq!(editor.cursor().visual_col, 3);

    // Reversibility: Left then Right should return to same index
    editor.move_cursor_left();
    assert_eq!(editor.cursor().byte_index, 2);
    assert_eq!(editor.cursor().visual_col, 2);

    editor.move_cursor_right();
    assert_eq!(editor.cursor().byte_index, 3);
    assert_eq!(editor.cursor().visual_col, 3);

    // Reversibility should NOT mutate the buffer contents or length
    assert_eq!(editor.text(), "abc");
}

#[test]
fn test_editor_boundary_deletes() {
    let mut editor = Editor::new();
    
    // Backspacing on empty buffer is a safe no-op
    editor.backspace();
    assert_eq!(editor.text(), "");
    assert_eq!(editor.cursor().byte_index, 0);

    // Deleting on empty buffer is a safe no-op
    editor.delete();
    assert_eq!(editor.text(), "");
    assert_eq!(editor.cursor().byte_index, 0);

    editor.insert('x');
    editor.insert('y');
    
    // Cursor is at end (idx 2). Deleting at end is a safe no-op
    editor.delete();
    assert_eq!(editor.text(), "xy");
    assert_eq!(editor.cursor().byte_index, 2);

    // Move left and backspace
    editor.move_cursor_left();
    editor.backspace();
    assert_eq!(editor.text(), "y");
    assert_eq!(editor.cursor().byte_index, 0);
}

#[test]
fn test_editor_empty_invariants() {
    let mut editor = Editor::new();
    // Verify arbitrary sequence of movements/deletions on empty buffer keeps state stable
    for _ in 0..10 {
        editor.move_cursor_left();
        editor.backspace();
        editor.delete();
        editor.move_cursor_right();
    }
    assert_eq!(editor.text(), "");
    assert_eq!(editor.cursor().byte_index, 0);
    assert_eq!(editor.cursor().visual_col, 0);
}

#[test]
fn test_scroll_clamps() {
    let mut scroll = ScrollState::new();
    scroll.scroll_up(); // unpins so offset stays 0
    scroll.update_bounds(10, 3); // max_offset = 7
    assert_eq!(scroll.max_offset(), 7);

    // Clamp at 0 when scrolling up at offset 0
    scroll.scroll_up();
    assert_eq!(scroll.offset(), 0);

    // Scroll down multiple times, clamping at max_offset
    for _ in 0..15 {
        scroll.scroll_down();
    }
    assert_eq!(scroll.offset(), 7);

    // Scroll up works
    scroll.scroll_up();
    assert_eq!(scroll.offset(), 6);
}

#[test]
fn test_dispatcher_exit_needs_render() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);

    let mut sidebar = SidebarInteraction::new();
    let visible_ids = vec![];
    let lookup = DummyLookup;

    // Exit action sets should_exit = true
    let res = Dispatcher::dispatch(
        InputAction::Command(Command::Exit),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            visible_ids: &visible_ids,
            lookup: &lookup,
        }
    );
    assert_eq!(
        res,
        DispatchResult { needs_render: false, should_exit: true, ui_event: None }
    );

    // FocusNext action sets needs_render = true
    let res = Dispatcher::dispatch(
        InputAction::Command(Command::FocusNext),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            visible_ids: &visible_ids,
            lookup: &lookup,
        }
    );
    assert_eq!(
        res,
        DispatchResult { needs_render: true, should_exit: false, ui_event: None }
    );

    // TextInput action sets needs_render = true
    let res = Dispatcher::dispatch(
        InputAction::Text(TextInput::Char('z')),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            visible_ids: &visible_ids,
            lookup: &lookup,
        }
    );
    assert_eq!(
        res,
        DispatchResult { needs_render: true, should_exit: false, ui_event: None }
    );
    assert_eq!(editor.text(), "z");
}

#[test]
fn test_separation_of_concerns() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);

    scroll.scroll_up(); // unpins so offset starts at 0
    scroll.update_bounds(20, 5); // max_offset = 15
    editor.insert('h');
    editor.insert('i');

    let mut sidebar = SidebarInteraction::new();
    let visible_ids = vec![];
    let lookup = DummyLookup;

    // Scroll operations must NOT modify editor state or cursor state
    let original_text = editor.text().to_string();
    let original_cursor = editor.cursor();
    
    Dispatcher::dispatch(
        InputAction::Command(Command::ScrollDown),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            visible_ids: &visible_ids,
            lookup: &lookup,
        }
    );
    assert_eq!(scroll.offset(), 1);
    assert_eq!(editor.text(), original_text);
    assert_eq!(editor.cursor(), original_cursor);

    // Editor cursor operations must NOT modify scroll state
    let original_offset = scroll.offset();
    Dispatcher::dispatch(
        InputAction::Command(Command::MoveLeft),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            visible_ids: &visible_ids,
            lookup: &lookup,
        }
    );
    assert_eq!(editor.cursor().byte_index, 1);
    assert_eq!(scroll.offset(), original_offset);
}
