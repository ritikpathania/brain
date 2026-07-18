//! Border layout and frame drawing helpers.

use crate::ui::render::context::RenderContext;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use ratatui::widgets::{Block, BorderType, Borders};

/// Helper to render borders consistently across widgets.
pub struct BorderRenderer;

// NOTE: Block is returned temporarily for M1. We will refactor this to draw
// borders directly onto the Buffer in a later milestone to match all primitives.
impl BorderRenderer {
    /// Constructs a rounded themed border block.
    pub fn rounded<T: ActiveTheme>(title: &str, ctx: &RenderContext<'_, T>) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_style(ctx.theme.style(ThemeToken::Muted))
            .border_type(BorderType::Rounded)
            .title(title.to_string())
    }

    /// Constructs a simple flat ASCII border block.
    pub fn plain<T: ActiveTheme>(title: &str, ctx: &RenderContext<'_, T>) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_style(ctx.theme.style(ThemeToken::Muted))
            .border_type(BorderType::Plain)
            .title(title.to_string())
    }
}
