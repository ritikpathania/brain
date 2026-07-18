//! Border panel container widget.

use crate::ui::render::{BorderRenderer, RenderContext};
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::widgets::brain_widget::BrainWidget;
use crate::ui::widgets::view_models::{FocusState, PanelView};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Precomputed layout bounds for the Panel.
pub struct PanelLayout {
    /// Area bounds inside the panel border lines.
    pub inner_area: Rect,
}

impl PanelLayout {
    /// Computes inner boundary coordinates for a panel area.
    pub fn compute(area: Rect) -> Self {
        let inner_area = if area.width > 2 && area.height > 2 {
            Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2)
        } else {
            Rect::new(area.x, area.y, 0, 0)
        };
        Self { inner_area }
    }
}

/// The Panel container widget renderer.
pub struct Panel<'a> {
    /// Reference to the immutable panel view model state.
    pub view: &'a PanelView<'a>,
}

impl<'a> BrainWidget for Panel<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let border_token = match self.view.focus {
            FocusState::Focused => ThemeToken::Primary,
            FocusState::Inactive => ThemeToken::Muted,
            FocusState::Disabled => ThemeToken::Muted,
        };
        let block = BorderRenderer::rounded(self.view.title, ctx)
            .border_style(ctx.theme.style(border_token));
        ratatui::widgets::Widget::render(block, area, buf);
    }
}
