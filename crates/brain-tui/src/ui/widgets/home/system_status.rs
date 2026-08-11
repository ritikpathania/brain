//! Engine Telemetry & System Status dashboard widget.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Renders System Status & Telemetry card on Home dashboard.
pub struct SystemStatusWidget {
    /// Active IPC daemon connection status flag.
    pub is_connected: bool,
    /// Measured round-trip query latency in milliseconds.
    pub latency_ms: u64,
}

impl Default for SystemStatusWidget {
    fn default() -> Self {
        Self {
            is_connected: true,
            latency_ms: 23,
        }
    }
}

impl SystemStatusWidget {
    /// Renders 2-column engine telemetry and UDS status metrics into target area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 20 || area.height < 4 {
            return;
        }

        let title_line = ratatui::text::Line::from(vec![ratatui::text::Span::styled(
            "System Status",
            theme
                .style(ThemeToken::HeaderPrimary)
                .add_modifier(Modifier::BOLD),
        )]);

        let status_str = if self.is_connected {
            "Online"
        } else {
            "Offline"
        };
        let status_style = if self.is_connected {
            theme.style(ThemeToken::Success)
        } else {
            theme.style(ThemeToken::Danger)
        };

        let lines = vec![
            title_line,
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("Daemon      ", theme.style(ThemeToken::TextSecondary)),
                ratatui::text::Span::styled(status_str, status_style),
            ]),
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("Latency     ", theme.style(ThemeToken::TextSecondary)),
                ratatui::text::Span::styled(
                    format!("{} ms", self.latency_ms),
                    theme.style(ThemeToken::TextPrimary),
                ),
            ]),
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("Engine      ", theme.style(ThemeToken::TextSecondary)),
                ratatui::text::Span::styled(
                    "Relational Memory",
                    theme.style(ThemeToken::TextPrimary),
                ),
            ]),
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("Storage     ", theme.style(ThemeToken::TextSecondary)),
                ratatui::text::Span::styled(
                    "SQLite FTS5 + Vector",
                    theme.style(ThemeToken::TextPrimary),
                ),
            ]),
        ];

        let p = Paragraph::new(lines);
        p.render(area, buf);
    }
}
