//! Allocation-free Divider primitives.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::render::context::RenderContext;

/// A simple horizontal divider line.
pub struct Divider;

impl Divider {
    /// Draws the divider directly to the buffer without allocation.
    pub fn draw<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let style = ctx.theme.style(ThemeToken::Muted);
        for x in area.left()..area.right() {
            buf.get_mut(x, area.y).set_symbol("─").set_style(style);
        }
    }
}
