//! Layout and geometry calculation module.

pub mod cell_width;
pub mod chat_screen;
pub mod dialog;
pub mod engine;
pub mod overlay;
pub mod spacing;
pub mod status_bar;

pub use cell_width::CellWidth;
pub use chat_screen::{ChatScreenGeometry, ResponsiveProfile, SIDEBAR_BREAKPOINT, SIDEBAR_WIDTH};
pub use dialog::{DialogGeometry, DialogMeasure};
pub use engine::LayoutEngine;
pub use overlay::{CommandPaletteGeometry, Overlay, SlashCompletionGeometry};
pub use spacing::Spacing;
pub use status_bar::{StatusBarGeometry, StatusBarMeasure};
