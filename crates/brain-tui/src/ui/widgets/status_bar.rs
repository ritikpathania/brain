//! StatusBar widget drawing titles, statuses, and animation ticks.

use crate::ui::layout::{CellWidth, LayoutEngine, StatusBarMeasure};
use crate::ui::primitives::{Spinner, SpinnerStyle};
use crate::ui::render::context::RenderContext;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::widgets::brain_widget::BrainWidget;
use crate::ui::widgets::view_models::{StatusBarView, StatusKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// The StatusBar widget renderer.
pub struct StatusBar<'a> {
    /// Reference to the immutable status bar view model state.
    pub view: &'a StatusBarView<'a>,
}

impl<'a> BrainWidget for StatusBar<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let measure = StatusBarMeasure {
            title_width: CellWidth::measure(self.view.title),
            show_spinner: matches!(self.view.kind, StatusKind::Working),
        };
        let geometry = LayoutEngine::status_bar(area, &measure);

        // Paint background
        buf.set_style(area, ctx.theme.style(ThemeToken::Background));

        // Paint title
        buf.set_stringn(
            geometry.title_area.x,
            geometry.title_area.y,
            self.view.title,
            geometry.title_area.width as usize,
            ctx.theme.style(ThemeToken::Primary),
        );

        // Paint optional spinner
        if geometry.spinner_area.width > 0 {
            let spinner = Spinner {
                style: SpinnerStyle::Thinking,
            };
            spinner.draw(geometry.spinner_area, buf, ctx);
        }

        // Paint status message
        let token = match self.view.kind {
            StatusKind::Idle => ThemeToken::Muted,
            StatusKind::Working => ThemeToken::Thinking,
            StatusKind::Streaming => ThemeToken::Streaming,
            StatusKind::Error => ThemeToken::Danger,
            StatusKind::Offline => ThemeToken::Muted,
        };
        buf.set_stringn(
            geometry.status_area.x,
            geometry.status_area.y,
            self.view.message,
            geometry.status_area.width as usize,
            ctx.theme.style(token),
        );
    }
}
