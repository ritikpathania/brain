//! StatusBar geometry configurations.

use ratatui::layout::Rect;
use crate::ui::layout::cell_width::CellWidth;

/// Pure numeric sizing parameters for the StatusBar layout.
pub struct StatusBarMeasure {
    /// Terminal cell display width of the title.
    pub title_width: CellWidth,
    /// Whether the spinner needs to be displayed.
    pub show_spinner: bool,
}

/// Immutable, copyable coordinate geometry output for StatusBar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusBarGeometry {
    /// Bounds for title string.
    pub title_area: Rect,
    /// Bounds for spinner tick indicator.
    pub spinner_area: Rect,
    /// Bounds for status connection message.
    pub status_area: Rect,
}
