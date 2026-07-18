//! Allocation-free Badge primitives.

use crate::ui::render::context::RenderContext;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// A pill or badge primitive for displaying small status labels.
pub struct Badge<'a> {
    /// The string label of the badge.
    pub label: &'a str,
    /// The semantic color token for the badge.
    pub token: ThemeToken,
}

impl<'a> Badge<'a> {
    /// Draws the badge to the screen buffer without heap allocation.
    pub fn draw<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        if area.width < 4 {
            return;
        }
        let style = ctx.theme.style(self.token);
        let muted = ctx.theme.style(ThemeToken::Muted);

        buf.set_string(area.x, area.y, " [", muted);
        let label_max = (area.width - 4) as usize;
        let final_len = self.label.len().min(label_max);
        buf.set_stringn(area.x + 2, area.y, self.label, final_len, style);
        buf.set_string(area.x + 2 + final_len as u16, area.y, "] ", muted);
    }
}
