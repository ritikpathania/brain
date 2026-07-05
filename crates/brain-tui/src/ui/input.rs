//! Input translation and event mapping.

use crossterm::event::{KeyEvent, KeyCode, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};

/// Global or layout actions that can be triggered by terminal input events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Request exit.
    Exit,
    /// Shift focus forward.
    FocusNext,
    /// Shift focus backward.
    FocusPrevious,
    /// Delete character behind cursor.
    Backspace,
    /// Delete character under cursor.
    Delete,
    /// Move cursor left.
    MoveLeft,
    /// Move cursor right.
    MoveRight,
    /// Scroll viewport up.
    ScrollUp,
    /// Scroll viewport down.
    ScrollDown,
    /// Submit current prompt.
    Submit,
    /// Toggle Command Palette overlay.
    ToggleCommandPalette,
    /// Dismiss or escape current transient view.
    Escape,
}


/// Character text input components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInput {
    /// Plain unicode character.
    Char(char),
}

/// Abstract actions that can be triggered by terminal input events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    /// Command actions.
    Command(Command),
    /// Text input characters.
    Text(TextInput),
    /// No action resolved.
    None,
}

/// Router that resolves raw events to semantic actions.
pub struct InputRouter;

impl InputRouter {
    /// Maps key events to InputActions terminal-agnostically.
    pub fn handle(key: KeyEvent) -> InputAction {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') | KeyCode::Char('C') => InputAction::Command(Command::Exit),
                KeyCode::Char('k') | KeyCode::Char('K') => InputAction::Command(Command::ToggleCommandPalette),
                _ => InputAction::None,
            }

        } else {
            match key.code {
                KeyCode::Esc => InputAction::Command(Command::Escape),
                KeyCode::Tab => InputAction::Command(Command::FocusNext),
                KeyCode::BackTab => InputAction::Command(Command::FocusPrevious),

                KeyCode::Backspace => InputAction::Command(Command::Backspace),
                KeyCode::Delete => InputAction::Command(Command::Delete),
                KeyCode::Left => InputAction::Command(Command::MoveLeft),
                KeyCode::Right => InputAction::Command(Command::MoveRight),
                KeyCode::Up => InputAction::Command(Command::ScrollUp),
                KeyCode::Down => InputAction::Command(Command::ScrollDown),
                KeyCode::Enter => InputAction::Command(Command::Submit),
                KeyCode::Char(c) => InputAction::Text(TextInput::Char(c)),
                _ => InputAction::None,
            }
        }
    }
}

/// Semantic mouse actions resolved from raw mouse events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    /// Request shift of input focus to a specific region.
    FocusRegion(crate::state::FocusRegion),
    /// Select a session by its unique ID.
    SelectSession(brain_domain::SessionId),
    /// Scroll viewport by a delta.
    Scroll(i32),
}

/// Router translating raw crossterm MouseEvents into semantic MouseActions.
pub struct MouseRouter;

impl MouseRouter {
    /// Resolves raw mouse events into semantic intents given the active geometry layout and sessions list.
    pub fn handle(
        event: MouseEvent,
        geometry: &crate::ui::layout::ChatScreenGeometry,
        sessions: &[brain_domain::SessionId],
    ) -> Option<MouseAction> {
        let col = event.column;
        let row = event.row;

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if click was inside prompt_area
                if col >= geometry.prompt_area.x
                    && col < geometry.prompt_area.x + geometry.prompt_area.width
                    && row >= geometry.prompt_area.y
                    && row < geometry.prompt_area.y + geometry.prompt_area.height
                {
                    return Some(MouseAction::FocusRegion(crate::state::FocusRegion::Editor));
                }

                // Check if click was inside sidebar_area
                if geometry.sidebar_area.width > 0
                    && col >= geometry.sidebar_area.x
                    && col < geometry.sidebar_area.x + geometry.sidebar_area.width
                    && row >= geometry.sidebar_area.y
                    && row < geometry.sidebar_area.y + geometry.sidebar_area.height
                {
                    // Calculate which row relative to the sidebar start was clicked.
                    // The sidebar header borders occupy row 0, so actual content starts at y + 1.
                    let relative_row = row.saturating_sub(geometry.sidebar_area.y + 1) as usize;
                    if relative_row < sessions.len() {
                        return Some(MouseAction::SelectSession(sessions[relative_row]));
                    }
                    return Some(MouseAction::FocusRegion(crate::state::FocusRegion::Sidebar));
                }

                None
            }
            MouseEventKind::ScrollUp => {
                if col >= geometry.chat_viewport_area.x
                    && col < geometry.chat_viewport_area.x + geometry.chat_viewport_area.width
                    && row >= geometry.chat_viewport_area.y
                    && row < geometry.chat_viewport_area.y + geometry.chat_viewport_area.height
                {
                    return Some(MouseAction::Scroll(-1));
                }
                None
            }
            MouseEventKind::ScrollDown => {
                if col >= geometry.chat_viewport_area.x
                    && col < geometry.chat_viewport_area.x + geometry.chat_viewport_area.width
                    && row >= geometry.chat_viewport_area.y
                    && row < geometry.chat_viewport_area.y + geometry.chat_viewport_area.height
                {
                    return Some(MouseAction::Scroll(1));
                }
                None
            }
            _ => None,
        }
    }
}

