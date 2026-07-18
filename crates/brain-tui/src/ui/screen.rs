//! Base contract for TUI Screen composition layout layers.

use crate::ui::render::context::RenderContext;
use crate::ui::theme::ActiveTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// A high-level view screen composition layer.
pub trait Screen {
    /// Renders the composed layout and children into the buffer.
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>);

    /// Returns static descriptive screen header title metadata.
    fn title(&self) -> &'static str {
        "Chat"
    }
}
