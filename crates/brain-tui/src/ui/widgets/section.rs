//! Section container toggle widget.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::render::{RenderContext, UnicodeSupport};
use crate::ui::primitives::Label;
use crate::ui::widgets::brain_widget::BrainWidget;
use crate::ui::widgets::view_models::SectionView;

/// Precomputed layout bounds for the Section.
pub struct SectionLayout {
    /// Area bounds for the expand/collapse indicator arrow.
    pub arrow_area: Rect,
    /// Area bounds for the section title text.
    pub title_area: Rect,
}

impl SectionLayout {
    /// Computes component layout bounds for the section.
    pub fn compute(area: Rect) -> Self {
        let arrow_area = Rect::new(area.x, area.y, 3, 1);
        let title_area = Rect::new(area.x + 3, area.y, area.width.saturating_sub(3), 1);
        Self { arrow_area, title_area }
    }
}

/// A collapsible drawer widget.
pub struct Section<'a> {
    /// Reference to the immutable section view model state.
    pub view: &'a SectionView<'a>,
}

impl<'a> BrainWidget for Section<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let layout = SectionLayout::compute(area);
        
        let arrow = if self.view.collapsed {
            if matches!(ctx.capabilities.unicode, UnicodeSupport::Full) { " ▶ " } else { " > " }
        } else {
            if matches!(ctx.capabilities.unicode, UnicodeSupport::Full) { " ▼ " } else { " v " }
        };

        buf.set_string(layout.arrow_area.x, layout.arrow_area.y, arrow, ctx.theme.style(ThemeToken::Muted));
        
        let label = Label { text: self.view.title, token: ThemeToken::Primary };
        label.draw(layout.title_area, buf, ctx);
    }
}
