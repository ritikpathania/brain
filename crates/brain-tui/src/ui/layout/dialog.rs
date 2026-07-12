//! Dialog geometry configurations.

use ratatui::layout::Rect;
use crate::ui::layout::cell_width::CellWidth;
use crate::ui::widgets::view_models::MAX_DIALOG_BUTTONS;

/// Pure numeric sizing parameters for the Dialog layout.
pub struct DialogMeasure<'a> {
    /// Button cell widths.
    pub button_widths: &'a [CellWidth],
}

/// Immutable, copyable coordinate geometry output for Dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogGeometry {
    /// Inner container bounds.
    pub inner_area: Rect,
    /// Message string bounds.
    pub message_area: Rect,
    pub(crate) button_areas: [Rect; MAX_DIALOG_BUTTONS],
    pub(crate) button_areas_len: usize,
}

impl DialogGeometry {
    /// Access active button layout bounds.
    pub fn button_areas(&self) -> &[Rect] {
        &self.button_areas[..self.button_areas_len]
    }
}
