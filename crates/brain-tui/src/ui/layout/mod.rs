//! Layout and geometry calculation module.

pub mod cell_width;
pub mod spacing;
pub mod engine;
pub mod status_bar;
pub mod dialog;
pub mod chat_screen;
pub mod overlay;

pub use cell_width::CellWidth;
pub use spacing::Spacing;
pub use engine::LayoutEngine;
pub use status_bar::{StatusBarMeasure, StatusBarGeometry};
pub use dialog::{DialogMeasure, DialogGeometry};
pub use chat_screen::{ResponsiveProfile, ChatScreenGeometry, SIDEBAR_WIDTH, SIDEBAR_BREAKPOINT};
pub use overlay::{Overlay, CommandPaletteGeometry, SlashCompletionGeometry};

