//! Welcome tagline widget — distinctive branded copy only.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Renders the branded Home tagline — no instructions, no explanation.
pub struct WelcomeWidget {
    /// Flag indicating whether this is the user's first launch (reserved for future use).
    pub is_first_launch: bool,
}

impl WelcomeWidget {
    /// Renders iconic brand tagline into buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let lines = vec![ratatui::text::Line::from(ratatui::text::Span::styled(
            "Think once. Remember forever.",
            theme
                .style(ThemeToken::TextPrimary)
                .add_modifier(Modifier::BOLD),
        ))];

        let p = Paragraph::new(lines).alignment(Alignment::Center);
        p.render(area, buf);
    }
}
