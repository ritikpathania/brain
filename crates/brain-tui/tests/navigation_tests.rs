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
