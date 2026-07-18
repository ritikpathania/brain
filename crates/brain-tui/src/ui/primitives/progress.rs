//! Allocation-free Progress bar primitives.

use crate::ui::render::context::RenderContext;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// A simple horizontal progress bar.
pub struct Progress {
    /// Completion ratio between 0.0 and 1.0.
    pub ratio: f32,
    /// Semantic token for the filled bar style.
    pub token: ThemeToken,
}

impl Progress {
    /// Draws the progress bar directly to the buffer without allocation.
    pub fn draw<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let style = ctx.theme.style(self.token);
        let muted = ctx.theme.style(ThemeToken::Muted);
        let width = area.width as usize;
        let filled_width = ((width as f32) * self.ratio.clamp(0.0, 1.0)) as usize;

        for x in 0..width {
            let cell = buf.get_mut(area.x + x as u16, area.y);
            if x < filled_width {
                cell.set_symbol("█").set_style(style);
            } else {
                cell.set_symbol("░").set_style(muted);
            }
        }
    }
}
