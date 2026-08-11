//! Global shortcut key mapping table.

use super::modal::Modal;
use super::screen::Screen;

/// High-level semantic UI events emitted from key shortcuts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    /// Navigate to a top-level screen target.
    NavigateScreen(Screen),
    /// Open a modal overlay target.
    OpenModal(Modal),
    /// Dismiss active modal or pop navigation stack.
    Cancel,
    /// Confirm selection or action.
    Confirm,
    /// Focus navigation move down.
    NavigateDown,
    /// Focus navigation move up.
    NavigateUp,
    /// Search query string modified.
    SearchChanged(String),
}

/// Evaluates raw key inputs against global desktop shortcut mappings.
pub struct GlobalShortcutMap;

impl GlobalShortcutMap {
    /// Maps key combinations to semantic UI events.
    pub fn evaluate(key: crossterm::event::KeyEvent) -> Option<UiEvent> {
        use crossterm::event::{KeyCode, KeyModifiers};

        match (key.code, key.modifiers) {
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                Some(UiEvent::OpenModal(Modal::CommandPalette))
            }
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                Some(UiEvent::OpenModal(Modal::CommandPalette))
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                Some(UiEvent::NavigateScreen(Screen::Home))
            }
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                Some(UiEvent::NavigateScreen(Screen::GraphExplorer))
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                Some(UiEvent::NavigateScreen(Screen::Reflection))
            }
            (KeyCode::Char(','), KeyModifiers::CONTROL) => {
                Some(UiEvent::NavigateScreen(Screen::Settings))
            }
            (KeyCode::Esc, _) => Some(UiEvent::Cancel),
            (KeyCode::Enter, _) => Some(UiEvent::Confirm),
            (KeyCode::Down, _) => Some(UiEvent::NavigateDown),
            (KeyCode::Up, _) => Some(UiEvent::NavigateUp),
            _ => None,
        }
    }
}
