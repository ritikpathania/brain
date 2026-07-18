//! Reusable layout solvers and coordinate mapping engines.

use crate::ui::layout::cell_width::CellWidth;
use crate::ui::layout::chat_screen::{ChatScreenGeometry, ResponsiveProfile, SIDEBAR_WIDTH};
use crate::ui::layout::dialog::{DialogGeometry, DialogMeasure};
use crate::ui::layout::spacing::Spacing;
use crate::ui::layout::status_bar::{StatusBarGeometry, StatusBarMeasure};
use crate::ui::widgets::view_models::MAX_DIALOG_BUTTONS;
use ratatui::layout::{Constraint, Layout, Rect};

/// Central layout coordinate computation engine.
pub struct LayoutEngine;

impl LayoutEngine {
    /// Applies uniform padding around a boundary.
    pub fn padding(area: Rect, top: u16, right: u16, bottom: u16, left: u16) -> Rect {
        Rect::new(
            area.x + left,
            area.y + top,
            area.width.saturating_sub(left + right),
            area.height.saturating_sub(top + bottom),
        )
    }

    /// Centered sub-bounds of specific dimensions.
    pub fn center(area: Rect, width: u16, height: u16) -> Rect {
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        Rect::new(x, y, width.min(area.width), height.min(area.height))
    }

    /// Solves geometry for the StatusBar widget.
    pub fn status_bar(area: Rect, measure: &StatusBarMeasure) -> StatusBarGeometry {
        let spinner_w = if measure.show_spinner { 2 } else { 0 };
        let parts = Layout::horizontal([
            Constraint::Length(measure.title_width.0),
            Constraint::Length(spinner_w),
            Constraint::Min(0),
        ])
        .split(area);

        StatusBarGeometry {
            title_area: parts[0],
            spinner_area: parts[1],
            status_area: parts[2],
        }
    }

    /// Solves geometry for the Dialog widget.
    pub fn dialog(area: Rect, measure: &DialogMeasure<'_>) -> DialogGeometry {
        let inner_area = Self::padding(area, 1, 1, 1, 1);
        let message_area = Rect::new(inner_area.x, inner_area.y, inner_area.width, 1);

        let mut button_areas = [Rect::default(); MAX_DIALOG_BUTTONS];
        let mut bx = inner_area.x;
        let by = inner_area.y + 2;
        let mut button_areas_len = 0;

        for &w in measure.button_widths.iter().take(MAX_DIALOG_BUTTONS) {
            let width = w.0 + 4; // Badge decoration pad
            button_areas[button_areas_len] =
                Rect::new(bx, by, width.min(inner_area.right().saturating_sub(bx)), 1);
            bx += width + Spacing::Normal.cells();
            button_areas_len += 1;
        }
        DialogGeometry {
            inner_area,
            message_area,
            button_areas,
            button_areas_len,
        }
    }

    /// Computes responsive geometry partition boundaries for the ChatScreen layout.
    pub fn chat_screen(area: Rect) -> ChatScreenGeometry {
        let profile = ResponsiveProfile::from_width(CellWidth(area.width));

        // Top Status Bar: Height 1
        let status_bar_area = Rect::new(area.x, area.y, area.width, 1.min(area.height));

        // Bottom Footer: Height 1
        let footer_y = area.bottom().saturating_sub(1);
        let footer_area = Rect::new(area.x, footer_y, area.width, 1.min(area.height));

        // Prompt Input: Height 3, right above footer
        let prompt_h = 3.min(area.height.saturating_sub(2));
        let prompt_y = footer_y.saturating_sub(prompt_h);
        let prompt_area = Rect::new(area.x, prompt_y, area.width, prompt_h);

        // Body area: remaining vertical space between status bar and prompt input
        let body_y = status_bar_area.bottom();
        let body_h = prompt_area.y.saturating_sub(body_y);
        let body_area = Rect::new(area.x, body_y, area.width, body_h);

        // Sidebar and Chat viewport horizontal split
        let (sidebar_area, chat_viewport_area) = match profile {
            ResponsiveProfile::Standard => {
                let sidebar_w = SIDEBAR_WIDTH.0.min(body_area.width);
                let sidebar = Rect::new(body_area.x, body_area.y, sidebar_w, body_area.height);
                let chat = Rect::new(
                    body_area.x + sidebar_w,
                    body_area.y,
                    body_area.width.saturating_sub(sidebar_w),
                    body_area.height,
                );
                (sidebar, chat)
            }
            ResponsiveProfile::Compact => {
                let sidebar = Rect::new(body_area.x, body_area.y, 0, body_area.height);
                let chat = body_area;
                (sidebar, chat)
            }
        };

        ChatScreenGeometry {
            profile,
            status_bar_area,
            sidebar_area,
            chat_viewport_area,
            prompt_area,
            footer_area,
        }
    }
}
