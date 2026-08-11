//! Memory Core Character Mascot with static logo + animated status indicator.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Stateful representation of the Brain runtime status for the status indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MascotState {
    /// Cold startup / Ready state.
    #[default]
    Ready,
    /// Active reasoning / thinking state.
    Reasoning,
    /// Memory search / retrieval state.
    Searching,
    /// Operation completed / knowledge synchronized.
    Success,
    /// Daemon disconnected / offline state.
    Disconnected,
}

impl MascotState {
    /// Returns a spinner/status indicator character and label for the state.
    pub fn status_indicator(&self) -> (&'static str, &'static str) {
        match self {
            MascotState::Ready => ("●", "Ready"),
            MascotState::Reasoning => ("◓", "Reasoning"),
            MascotState::Searching => ("◐", "Searching"),
            MascotState::Success => ("✔", "Memory updated"),
            MascotState::Disconnected => ("○", "Disconnected"),
        }
    }
}

/// The static Memory Core character logo — exact silhouette matching Claude mascot art.
pub const MASCOT_LOGO: [&str; 3] = [" ▐▛███▜▌ ", "▝▜█████▛▘", "  ▘▘ ▝▝  "];

/// Renders the static Memory Core character ASCII mascot logo.
#[derive(Debug, Clone, Copy, Default)]
pub struct MascotWidget {
    /// Current runtime status state.
    pub state: MascotState,
}

impl MascotWidget {
    /// Renders the mascot in Ready state into the specified buffer area.
    pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme) {
        Self::render_with_state(area, buf, theme, MascotState::Ready);
    }

    /// Renders the static Memory Core logo with a tiny status indicator beneath.
    pub fn render_with_state(area: Rect, buf: &mut Buffer, theme: &Theme, _state: MascotState) {
        let lines = vec![
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(ratatui::text::Span::styled(
                "Welcome back!",
                theme
                    .style(ThemeToken::HeaderPrimary)
                    .add_modifier(Modifier::BOLD),
            )),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(ratatui::text::Span::styled(
                MASCOT_LOGO[0],
                theme.style(ThemeToken::HeaderPrimary),
            )),
            ratatui::text::Line::from(ratatui::text::Span::styled(
                MASCOT_LOGO[1],
                theme.style(ThemeToken::HeaderPrimary),
            )),
            ratatui::text::Line::from(ratatui::text::Span::styled(
                MASCOT_LOGO[2],
                theme.style(ThemeToken::HeaderPrimary),
            )),
            ratatui::text::Line::from(ratatui::text::Span::styled(
                "Think once. Remember forever.",
                theme.style(ThemeToken::TextSecondary),
            )),
        ];

        let widget = Paragraph::new(lines).alignment(Alignment::Center);
        widget.render(area, buf);
    }
}
