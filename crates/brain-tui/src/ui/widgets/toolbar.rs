//! Tab-based navigation Toolbar widget.

use crate::ui::primitives::Badge;
use crate::ui::render::RenderContext;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::widgets::brain_widget::BrainWidget;
use crate::ui::widgets::view_models::{ToolbarView, MAX_TABS};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Precomputed layout bounds for the Toolbar tab elements.
pub struct ToolbarLayout {
    tab_areas: [Rect; MAX_TABS],
    len: usize,
}

impl ToolbarLayout {
    /// Computes component layout bounds for the toolbar tab views.
    pub fn compute(area: Rect, view: &ToolbarView<'_>) -> Self {
        let mut tab_areas = [Rect::default(); MAX_TABS];
        let mut tx = area.x;
        let mut len = 0;
        for tab in view.tabs.iter().take(MAX_TABS) {
            let width = (tab.title.len() + 4) as u16;
            tab_areas[len] = Rect::new(tx, area.y, width.min(area.right().saturating_sub(tx)), 1);
            tx += width + 1;
            len += 1;
        }
        Self { tab_areas, len }
    }

    /// Access the precomputed tab layout areas.
    pub fn tab_areas(&self) -> &[Rect] {
        &self.tab_areas[..self.len]
    }
}

/// The Toolbar tabs widget renderer.
pub struct Toolbar<'a> {
    /// Reference to the immutable toolbar view model state.
    pub view: &'a ToolbarView<'a>,
}

impl<'a> BrainWidget for Toolbar<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let layout = ToolbarLayout::compute(area, self.view);
        let tab_areas = layout.tab_areas();

        for idx in 0..tab_areas.len() {
            let tab = &self.view.tabs[idx];
            let token = if tab.active {
                ThemeToken::Accent
            } else {
                ThemeToken::Muted
            };
            let badge = Badge {
                label: tab.title,
                token,
            };
            badge.draw(tab_areas[idx], buf, ctx);
        }
    }
}
