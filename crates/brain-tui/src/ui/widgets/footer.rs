//! Footer widget presenting hotkeys list.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::render::RenderContext;
use crate::ui::primitives::{Badge, Label};
use crate::ui::widgets::brain_widget::BrainWidget;
use crate::ui::widgets::view_models::{FooterView, MAX_SHORTCUTS};

/// Precomputed layout bounds for the Footer shortcut links.
pub struct FooterLayout {
    shortcut_areas: [Rect; MAX_SHORTCUTS],
    len: usize,
}

impl FooterLayout {
    /// Computes the layout bounds given the available area and view parameters.
    pub fn compute(area: Rect, view: &FooterView<'_>) -> Self {
        let mut shortcut_areas = [Rect::default(); MAX_SHORTCUTS];
        let mut x = area.x;
        let mut len = 0;
        for shortcut in view.shortcuts.iter().take(MAX_SHORTCUTS) {
            let width = (shortcut.key.len() + shortcut.description.len() + 6) as u16;
            let current_w = width.min(area.right().saturating_sub(x));
            shortcut_areas[len] = Rect::new(x, area.y, current_w, 1);
            x += current_w;
            len += 1;
        }
        Self { shortcut_areas, len }
    }

    /// Access the precomputed shortcut areas.
    pub fn areas(&self) -> &[Rect] {
        &self.shortcut_areas[..self.len]
    }
}

/// The Footer widget renderer.
pub struct Footer<'a> {
    /// Reference to the immutable footer view model state.
    pub view: &'a FooterView<'a>,
}

impl<'a> BrainWidget for Footer<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let layout = FooterLayout::compute(area, self.view);
        let areas = layout.areas();
        
        for (idx, shortcut) in self.view.shortcuts.iter().enumerate() {
            if idx >= areas.len() {
                break;
            }
            let sub_area = areas[idx];
            if sub_area.width < 5 {
                continue;
            }
            
            let key_w = (shortcut.key.len() + 4) as u16;
            let badge = Badge { label: shortcut.key, token: ThemeToken::Primary };
            badge.draw(Rect::new(sub_area.x, sub_area.y, key_w.min(sub_area.width), 1), buf, ctx);
            
            if sub_area.width > key_w {
                let label = Label { text: shortcut.description, token: ThemeToken::Muted };
                label.draw(Rect::new(sub_area.x + key_w, sub_area.y, sub_area.width - key_w, 1), buf, ctx);
            }
        }
    }
}
