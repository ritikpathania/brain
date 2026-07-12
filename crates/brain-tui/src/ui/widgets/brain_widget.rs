//! Base rendering contract for all internal TUI widgets.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crate::ui::theme::ActiveTheme;
use crate::ui::render::context::RenderContext;

/// Base widget trait accepting the active theme and terminal capabilities context.
pub trait BrainWidget {
    /// Renders the widget into the buffer cells under the given context.
    /// Invariant: Must not mutate the ViewModel.
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>);
}
