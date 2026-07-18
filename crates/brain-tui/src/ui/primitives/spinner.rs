//! Allocation-free animated Spinner primitives.

use crate::ui::render::context::RenderContext;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Different semantic styles of spinners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinnerStyle {
    /// Active thinking state.
    Thinking,
    /// Streaming activity state.
    Working,
    /// Package downloading state.
    Downloading,
    /// Background indexing/updating state.
    Indexing,
}

/// An animated spinner primitive.
pub struct Spinner {
    /// Visual style of the spinner.
    pub style: SpinnerStyle,
}

impl Spinner {
    const BRAILLE_FRAMES: &'static [&'static str] =
        &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    const LINE_FRAMES: &'static [&'static str] = &["|", "/", "-", "\\"];

    /// Draws the spinner to the screen buffer. Frame transition depends on `ctx.tick`.
    pub fn draw<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let (frames, token) = match self.style {
            SpinnerStyle::Thinking => (Self::BRAILLE_FRAMES, ThemeToken::Thinking),
            SpinnerStyle::Working => (Self::LINE_FRAMES, ThemeToken::Streaming),
            SpinnerStyle::Downloading => (Self::LINE_FRAMES, ThemeToken::Secondary),
            SpinnerStyle::Indexing => (Self::LINE_FRAMES, ThemeToken::Accent),
        };
        let frame = frames[ctx.tick % frames.len()];
        buf.set_string(area.x, area.y, frame, ctx.theme.style(token));
    }
}
