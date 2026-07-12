//! ScrollView viewport viewport container widget.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::render::RenderContext;
use crate::ui::primitives::Label;
use crate::ui::widgets::brain_widget::BrainWidget;
use crate::ui::widgets::view_models::{ScrollViewModel, MAX_VISIBLE_SCROLL_ROWS};

/// Precomputed layout bounds for the ScrollView lines.
pub struct ScrollViewLayout {
    line_areas: [Rect; MAX_VISIBLE_SCROLL_ROWS],
    len: usize,
}

impl ScrollViewLayout {
    /// Computes layout coordinates for visible text lines in the scroll range.
    pub fn compute(area: Rect, view: &ScrollViewModel<'_>) -> Self {
        let mut line_areas = [Rect::default(); MAX_VISIBLE_SCROLL_ROWS];
        let height = (area.height as usize).min(MAX_VISIBLE_SCROLL_ROWS);
        let start = view.scroll_offset.min(view.lines.len());
        let end = (start + height).min(view.lines.len());
        let mut len = 0;
        for idx in 0..(end - start) {
            line_areas[len] = Rect::new(area.x, area.y + idx as u16, area.width, 1);
            len += 1;
        }
        Self { line_areas, len }
    }

    /// Access the precomputed scroll line areas.
    pub fn line_areas(&self) -> &[Rect] {
        &self.line_areas[..self.len]
    }
}

/// The ScrollView container widget renderer.
pub struct ScrollViewWidget<'a> {
    /// Reference to the immutable scroll view model state.
    pub view: &'a ScrollViewModel<'a>,
}

impl<'a> BrainWidget for ScrollViewWidget<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let layout = ScrollViewLayout::compute(area, self.view);
        let line_areas = layout.line_areas();
        let start = self.view.scroll_offset.min(self.view.lines.len());
        
        for idx in 0..line_areas.len() {
            let line = self.view.lines[start + idx];
            let label = Label { text: line, token: ThemeToken::Muted };
            label.draw(line_areas[idx], buf, ctx);
        }
    }
}
