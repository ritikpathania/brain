//! Generic vertical select list widget.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::render::RenderContext;
use crate::ui::primitives::Label;
use crate::ui::widgets::brain_widget::BrainWidget;
use crate::ui::widgets::view_models::{ListView, MAX_VISIBLE_LIST_ROWS};

/// Precomputed layout bounds for the List rows.
pub struct ListLayout {
    item_areas: [Rect; MAX_VISIBLE_LIST_ROWS],
    len: usize,
}

impl ListLayout {
    /// Computes component layout bounds for the visible list item rows.
    pub fn compute(area: Rect, view: &ListView<'_>) -> Self {
        let mut item_areas = [Rect::default(); MAX_VISIBLE_LIST_ROWS];
        let mut len = 0;
        for idx in 0..view.items.len().min(MAX_VISIBLE_LIST_ROWS) {
            let y = area.y + idx as u16;
            if y >= area.bottom() {
                break;
            }
            item_areas[len] = Rect::new(area.x, y, area.width, 1);
            len += 1;
        }
        Self { item_areas, len }
    }

    /// Access the precomputed list item areas.
    pub fn item_areas(&self) -> &[Rect] {
        &self.item_areas[..self.len]
    }
}

/// The List selection widget renderer.
pub struct List<'a> {
    /// Reference to the immutable list view model state.
    pub view: &'a ListView<'a>,
}

impl<'a> BrainWidget for List<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let layout = ListLayout::compute(area, self.view);
        let item_areas = layout.item_areas();
        
        for idx in 0..item_areas.len() {
            let item = &self.view.items[idx];
            let item_area = item_areas[idx];
            if item_area.width < 2 {
                continue;
            }
            
            let (indicator, token) = if item.selected {
                ("➔ ", ThemeToken::Primary)
            } else {
                ("  ", ThemeToken::Muted)
            };
            
            buf.set_string(item_area.x, item_area.y, indicator, ctx.theme.style(ThemeToken::Muted));
            
            let style_token = if item.disabled { ThemeToken::Muted } else { token };
            let label = Label { text: item.label, token: style_token };
            label.draw(Rect::new(item_area.x + 2, item_area.y, item_area.width - 2, 1), buf, ctx);
        }
    }
}
