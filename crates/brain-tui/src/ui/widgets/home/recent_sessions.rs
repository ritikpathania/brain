//! Recent sessions dashboard card widget.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget};

/// Renders the Recent Sessions summary list on the Home screen.
pub struct RecentSessionsWidget<'a> {
    /// Active sessions slice.
    pub sessions: &'a [crate::state::SessionViewModel],
}

impl<'a> RecentSessionsWidget<'a> {
    /// Renders recent session entries into the target buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = theme.panel(" Recent Sessions ", true);
        let mut lines = Vec::new();

        if self.sessions.is_empty() {
            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "No active sessions found.",
                theme.style(ThemeToken::TextMuted),
            )));
            lines.push(ratatui::text::Line::from(ratatui::text::Span::raw("")));
            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "Type /session new in Command Palette to start.",
                theme.style(ThemeToken::Accent),
            )));
        } else {
            for (idx, sess) in self.sessions.iter().take(4).enumerate() {
                let prefix = format!(" [{}] ", idx + 1);
                lines.push(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(prefix, theme.style(ThemeToken::Accent)),
                    ratatui::text::Span::styled(&sess.title, theme.style(ThemeToken::TextPrimary)),
                ]));
            }
        }

        let p = Paragraph::new(lines).block(block);
        p.render(area, buf);
    }
}
