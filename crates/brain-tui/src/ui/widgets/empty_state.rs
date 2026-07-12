//! EmptyState helper graphics widget.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::render::RenderContext;
use crate::ui::primitives::Label;
use crate::ui::widgets::brain_widget::BrainWidget;
use crate::ui::widgets::view_models::EmptyStateView;

/// Precomputed layout bounds for EmptyState components.
pub struct EmptyStateLayout {
    /// Area bounds for the central graphic/icon.
    pub icon_area: Rect,
    /// Area bounds for the primary heading.
    pub title_area: Rect,
    /// Area bounds for the descriptive body details.
    pub desc_area: Rect,
}

impl EmptyStateLayout {
    /// Computes component layout bounds for the empty state panel.
    pub fn compute(area: Rect) -> Self {
        let icon_area = Rect::new(area.x, area.y, area.width, 1);
        let title_area = Rect::new(area.x, area.y + 1, area.width, 1);
        let desc_area = Rect::new(area.x, area.y + 2, area.width, area.height.saturating_sub(2));
        Self { icon_area, title_area, desc_area }
    }
}

/// The EmptyState panel widget renderer.
pub struct EmptyState<'a> {
    /// Reference to the immutable empty state view model.
    pub view: &'a EmptyStateView<'a>,
}

impl<'a> BrainWidget for EmptyState<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let layout = EmptyStateLayout::compute(area);
        
        // Paint central icon
        let label_icon = Label { text: self.view.icon, token: ThemeToken::Muted };
        label_icon.draw(layout.icon_area, buf, ctx);

        // Paint title
        let label_title = Label { text: self.view.title, token: ThemeToken::Primary };
        label_title.draw(layout.title_area, buf, ctx);

        // Paint description
        let label_desc = Label { text: self.view.description, token: ThemeToken::Muted };
        label_desc.draw(layout.desc_area, buf, ctx);
    }
}
