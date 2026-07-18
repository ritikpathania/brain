//! Stateless Label primitives.

use crate::ui::render::context::RenderContext;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// A simple text label primitive.
pub struct Label<'a> {
    /// The string text to display.
    pub text: &'a str,
    /// The semantic style color token.
    pub token: ThemeToken,
}

impl<'a> Label<'a> {
    /// Draws the label directly to the buffer.
    pub fn draw<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let style = ctx.theme.style(self.token);
        buf.set_stringn(area.x, area.y, self.text, area.width as usize, style);
    }
}
