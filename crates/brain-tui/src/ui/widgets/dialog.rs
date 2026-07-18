//! Modal dialog popup widget.

use crate::ui::layout::{CellWidth, DialogMeasure, LayoutEngine};
use crate::ui::primitives::{Badge, Label};
use crate::ui::render::{BorderRenderer, RenderContext};
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::widgets::brain_widget::BrainWidget;
use crate::ui::widgets::view_models::{ButtonKind, DialogView, MAX_DIALOG_BUTTONS};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// The Dialog modal widget renderer.
pub struct Dialog<'a> {
    /// Reference to the immutable dialog view model state.
    pub view: &'a DialogView<'a>,
}

impl<'a> BrainWidget for Dialog<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let mut button_widths = [CellWidth(0); MAX_DIALOG_BUTTONS];
        let buttons_len = self.view.buttons.len().min(MAX_DIALOG_BUTTONS);
        for (idx, _) in self.view.buttons[..buttons_len].iter().enumerate() {
            button_widths[idx] = CellWidth::measure(self.view.buttons[idx].label);
        }
        let measure = DialogMeasure {
            button_widths: &button_widths[..buttons_len],
        };
        let geometry = LayoutEngine::dialog(area, &measure);
        let button_areas = geometry.button_areas();

        let block = BorderRenderer::rounded(self.view.title, ctx);
        ratatui::widgets::Widget::render(block, area, buf);

        if geometry.inner_area.width == 0 {
            return;
        }

        // Draw message
        let label = Label {
            text: self.view.message,
            token: ThemeToken::Muted,
        };
        label.draw(geometry.message_area, buf, ctx);

        // Draw choices
        for (idx, choice_area) in button_areas.iter().enumerate() {
            let button = &self.view.buttons[idx];
            let choice_area = *choice_area;
            let active = idx == self.view.selected_index;

            let token = if !button.enabled {
                ThemeToken::Muted
            } else if active {
                ThemeToken::Primary
            } else {
                match button.kind {
                    ButtonKind::Primary => ThemeToken::Accent,
                    ButtonKind::Secondary => ThemeToken::Muted,
                    ButtonKind::Danger => ThemeToken::Danger,
                }
            };

            let badge = Badge {
                label: button.label,
                token,
            };
            badge.draw(choice_area, buf, ctx);
        }
    }
}
