//! Allocation-free text rendering helpers.

use crate::ui::render::context::RenderContext;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Static helper to draw styled text chunks directly to the backend buffer.
pub struct TextRenderer;

impl TextRenderer {
    /// Draws a styled string to the screen buffer without heap allocation.
    pub fn draw<T: ActiveTheme>(
        buf: &mut Buffer,
        area: Rect,
        text: &str,
        token: ThemeToken,
        ctx: &RenderContext<'_, T>,
    ) {
        let style = ctx.theme.style(token);
        buf.set_stringn(area.x, area.y, text, area.width as usize, style);
    }
}
