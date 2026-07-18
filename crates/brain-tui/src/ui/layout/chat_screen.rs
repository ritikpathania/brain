//! ChatScreen responsive geometry configurations.

use crate::ui::layout::cell_width::CellWidth;
use ratatui::layout::Rect;

/// Sidebar cell width.
pub const SIDEBAR_WIDTH: CellWidth = CellWidth(25);

/// Breakpoint width separating compact from standard display profiles.
pub const SIDEBAR_BREAKPOINT: CellWidth = CellWidth(80);

/// Named responsive layout profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponsiveProfile {
    /// Narrow mobile/split-pane layout.
    Compact,
    /// Large standard layout.
    Standard,
}

impl ResponsiveProfile {
    /// Computes active layout profile based on parent display columns.
    pub fn from_width(width: CellWidth) -> Self {
        if width >= SIDEBAR_BREAKPOINT {
            ResponsiveProfile::Standard
        } else {
            ResponsiveProfile::Compact
        }
    }
}

/// Computed coordinate boundaries for chat screen layout panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatScreenGeometry {
    /// Profile tag.
    pub profile: ResponsiveProfile,
    /// Title banner area.
    pub status_bar_area: Rect,
    /// Sessions list area.
    pub sidebar_area: Rect,
    /// Messages viewport area.
    pub chat_viewport_area: Rect,
    /// Input prompt area.
    pub prompt_area: Rect,
    /// Bottom shortcut hints area.
    pub footer_area: Rect,
}
