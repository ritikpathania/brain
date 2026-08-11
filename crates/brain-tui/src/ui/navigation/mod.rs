//! Navigation, modal overlay, and focus routing subsystem.

pub mod modal;
pub mod screen;
pub mod shortcut_map;
pub mod stack;

pub use modal::Modal;
pub use screen::Screen;
pub use shortcut_map::{GlobalShortcutMap, UiEvent};
pub use stack::NavigationStack;
