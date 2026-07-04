//! Input dispatcher coordinating mutations on interaction context.

use crate::ui::input::{InputAction, Command, TextInput};
use crate::ui::interaction::editor::Editor;
use crate::ui::interaction::scroll::ScrollState;
use crate::ui::focus::FocusManager;
use crate::ui::widgets::view_models::FocusTarget;
use crate::ui::interaction::sidebar::{SidebarInteraction, SidebarEvent, SessionLookup};
use brain_domain::SessionId;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};

/// Collection of interaction sub-systems.
pub struct InteractionContext<'a> {
    /// Reference to the mutable text Editor.
    pub editor: &'a mut Editor,
    /// Reference to the mutable ScrollState.
    pub scroll: &'a mut ScrollState,
    /// Reference to the mutable FocusManager.
    pub focus: &'a mut FocusManager,
    /// Reference to the mutable SidebarInteraction.
    pub sidebar: &'a mut SidebarInteraction,
    /// The visible session IDs in the sidebar.
    pub visible_ids: &'a [SessionId],
    /// The session lookup service.
    pub lookup: &'a dyn SessionLookup,
}

/// Abstract user interface intent events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    /// Intent to submit prompt text.
    SubmitPrompt(String),
    /// Intent to resize terminal window.
    Resize(u16, u16),
    /// Intent from sidebar action.
    Sidebar(SidebarEvent),
}

/// Dispatcher result codes representing TUI state changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchResult {
    /// Request interface frame redrawing.
    pub needs_render: bool,
    /// Request client shutdown.
    pub should_exit: bool,
    /// Optional semantic application event emitted.
    pub ui_event: Option<UiEvent>,
}

impl DispatchResult {
    /// Instantiates a DispatchResult with no flags set.
    pub fn none() -> Self {
        Self { needs_render: false, should_exit: false, ui_event: None }
    }

    /// Instantiates a DispatchResult requesting redraw.
    pub fn render() -> Self {
        Self { needs_render: true, should_exit: false, ui_event: None }
    }

    /// Instantiates a DispatchResult requesting exit.
    pub fn exit() -> Self {
        Self { needs_render: false, should_exit: true, ui_event: None }
    }

    /// Instantiates a DispatchResult wrapping a UI event.
    pub fn event(event: UiEvent) -> Self {
        Self { needs_render: true, should_exit: false, ui_event: Some(event) }
    }
}

/// Central interaction routing manager.
pub struct Dispatcher;

impl Dispatcher {
    /// Executes the InputAction against the given sub-systems context.
    pub fn dispatch(action: InputAction, ctx: &mut InteractionContext<'_>) -> DispatchResult {
        if ctx.focus.current() == FocusTarget::Sidebar {
            let key_opt = match action {
                InputAction::Command(cmd) => match cmd {
                    Command::ScrollUp => Some(KeyEvent::new(KeyCode::Up, KeyModifiers::empty())),
                    Command::ScrollDown => Some(KeyEvent::new(KeyCode::Down, KeyModifiers::empty())),
                    Command::Submit => Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
                    Command::Backspace => Some(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())),
                    Command::Delete => Some(KeyEvent::new(KeyCode::Delete, KeyModifiers::empty())),
                    Command::MoveLeft => Some(KeyEvent::new(KeyCode::Left, KeyModifiers::empty())),
                    Command::MoveRight => Some(KeyEvent::new(KeyCode::Right, KeyModifiers::empty())),
                    _ => None,
                },
                InputAction::Text(TextInput::Char(c)) => Some(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty())),
                InputAction::None => None,
            };

            if let Some(key) = key_opt {
                let (handled, event) = ctx.sidebar.handle_key(key, ctx.visible_ids, ctx.lookup);
                if handled {
                    if let Some(sidebar_evt) = event {
                        return DispatchResult::event(UiEvent::Sidebar(sidebar_evt));
                    } else {
                        return DispatchResult::render();
                    }
                }
            }

            // If the sidebar is focused, prevent general editing and scrolling actions from falling through to the prompt editor or history scroll
            match action {
                InputAction::Command(cmd) => match cmd {
                    Command::Exit | Command::FocusNext | Command::FocusPrevious => {}
                    _ => return DispatchResult::none(),
                },
                InputAction::Text(_) => return DispatchResult::none(),
                InputAction::None => {}
            }
        }

        match action {
            InputAction::Command(cmd) => match cmd {
                Command::Exit => DispatchResult::exit(),
                Command::FocusNext => {
                    ctx.focus.next();
                    DispatchResult::render()
                }
                Command::FocusPrevious => {
                    ctx.focus.prev();
                    DispatchResult::render()
                }
                Command::Backspace => {
                    ctx.editor.backspace();
                    DispatchResult::render()
                }
                Command::Delete => {
                    ctx.editor.delete();
                    DispatchResult::render()
                }
                Command::MoveLeft => {
                    ctx.editor.move_cursor_left();
                    DispatchResult::render()
                }
                Command::MoveRight => {
                    ctx.editor.move_cursor_right();
                    DispatchResult::render()
                }
                Command::ScrollUp => {
                    ctx.scroll.scroll_up();
                    DispatchResult::render()
                }
                Command::ScrollDown => {
                    ctx.scroll.scroll_down();
                    DispatchResult::render()
                }
                Command::Submit => {
                    let text = ctx.editor.text().trim().to_string();
                    if text.is_empty() {
                        DispatchResult::none()
                    } else {
                        DispatchResult::event(UiEvent::SubmitPrompt(text))
                    }
                }
            },
            InputAction::Text(text_input) => match text_input {
                TextInput::Char(c) => {
                    ctx.editor.insert(c);
                    DispatchResult::render()
                }
            },
            InputAction::None => DispatchResult::none(),
        }
    }
}
