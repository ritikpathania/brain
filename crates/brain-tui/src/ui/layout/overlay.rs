//! Shared overlay abstraction and layout helpers.

use ratatui::layout::Rect;

/// Interface for transient layout overlays.
pub trait Overlay {
    /// Whether the overlay is currently visible.
    fn is_visible(&self) -> bool;
    /// Computes the overlay's layout boundaries.
    fn geometry(&self, screen_area: Rect) -> Rect;
}

/// Helper for solving Command Palette overlay coordinates.
pub struct CommandPaletteGeometry;

impl CommandPaletteGeometry {
    /// Computes centered bounds clamped to min/max dimensions.
    pub fn compute(terminal: Rect) -> Rect {
        let width = terminal.width.clamp(40, 80);
        let height = terminal.height.clamp(8, 15);

        let x = terminal.x + terminal.width.saturating_sub(width) / 2;
        let y = terminal.y + terminal.height.saturating_sub(height) / 2;
        Rect::new(x, y, width.min(terminal.width), height.min(terminal.height))
    }
}

/// Helper for solving Slash Completion overlay coordinates.
pub struct SlashCompletionGeometry;

impl SlashCompletionGeometry {
    /// Computes floating list bounds located above the prompt area.
    pub fn compute(terminal: Rect, prompt_area: Rect, item_count: usize) -> Rect {
        let height = (item_count as u16).clamp(3, 8);
        let width = prompt_area.width.clamp(30, 60);

        let x = prompt_area.x;
        let y = prompt_area.y.saturating_sub(height);

        Rect::new(x, y, width.min(terminal.width), height.min(terminal.height))
    }
}
