//! CommandHint suggestion popup helper.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::render::{RenderContext, BorderRenderer};
use crate::ui::primitives::Label;
use crate::ui::widgets::brain_widget::BrainWidget;
use crate::ui::widgets::view_models::CommandHintView;

/// Precomputed layout bounds for the CommandHint suggestions.
pub struct CommandHintLayout {
    /// Area bounds inside borders.
    pub inner_area: Rect,
    /// Area bounds for the matching command preview line.
    pub command_area: Rect,
    /// Area bounds for the usage parameters line.
    pub usage_area: Rect,
}

impl CommandHintLayout {
    /// Computes component layout bounds for the helper popup.
    pub fn compute(area: Rect) -> Self {
        let inner_area = if area.width > 2 && area.height > 2 {
            Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2)
        } else {
            Rect::new(area.x, area.y, 0, 0)
        };
        
        let command_area = Rect::new(inner_area.x, inner_area.y, inner_area.width, 1);
        let usage_area = Rect::new(inner_area.x, inner_area.y + 1, inner_area.width, 1);
        
        Self { inner_area, command_area, usage_area }
    }
}

/// The CommandHint popup widget renderer.
pub struct CommandHint<'a> {
    /// Reference to the immutable command hint view model state.
    pub view: &'a CommandHintView<'a>,
}

impl<'a> BrainWidget for CommandHint<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let layout = CommandHintLayout::compute(area);
        let block = BorderRenderer::rounded("Suggestion", ctx);
        ratatui::widgets::Widget::render(block, area, buf);
        
        if layout.inner_area.width == 0 {
            return;
        }

        let label_cmd = Label { text: self.view.command, token: ThemeToken::Primary };
        label_cmd.draw(layout.command_area, buf, ctx);
        
        let label_usage = Label { text: self.view.usage, token: ThemeToken::Muted };
        label_usage.draw(layout.usage_area, buf, ctx);
    }
}
