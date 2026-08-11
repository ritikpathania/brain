//! Lightweight two-column slash command launcher for the Welcome screen.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Structured representation of a quick action shortcut launcher item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickActionItem {
    /// Human-scannable verb title (e.g. "New Session") — used as fallback label.
    pub title: &'static str,
    /// Slash command string (e.g. "/session new").
    pub command: &'static str,
    /// Brief action description.
    pub description: &'static str,
}

impl QuickActionItem {
    /// Focused top 3 verb-first launcher items for the Welcome screen.
    pub fn default_actions() -> &'static [Self] {
        &[
            Self {
                title: "New Session",
                command: "/session new",
                description: "Start a new session",
            },
            Self {
                title: "Search Memory",
                command: "/search",
                description: "Search memories",
            },
            Self {
                title: "Shortcuts & Help",
                command: "/help",
                description: "View commands",
            },
        ]
    }
}

/// Renders lightweight two-column slash command launcher.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuickActionsWidget {
    /// Optional index of the currently highlighted/selected quick action.
    pub selected_index: Option<usize>,
}

impl QuickActionsWidget {
    /// Renders two-column slash command launcher as whitespace-separated rows without panel borders.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme, actions: &[QuickActionItem]) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let selected = self.selected_index.unwrap_or(0);
        let mut lines = Vec::with_capacity(actions.len() + 1);

        lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
            "Try",
            theme
                .style(ThemeToken::TextSecondary)
                .add_modifier(Modifier::BOLD),
        )));

        for (idx, action) in actions.iter().enumerate() {
            let is_selected = idx == selected;
            let cursor = if is_selected { "▶ " } else { "  " };
            let cmd_style = if is_selected {
                theme.style(ThemeToken::Accent).add_modifier(Modifier::BOLD)
            } else {
                theme.style(ThemeToken::TextPrimary)
            };
            let desc_style = if is_selected {
                theme.style(ThemeToken::TextSecondary)
            } else {
                theme.style(ThemeToken::TextMuted)
            };

            if area.width >= 48 {
                lines.push(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(cursor, theme.style(ThemeToken::Accent)),
                    ratatui::text::Span::styled(format!("{:<18}", action.command), cmd_style),
                    ratatui::text::Span::styled(action.description, desc_style),
                ]));
            } else {
                lines.push(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(cursor, theme.style(ThemeToken::Accent)),
                    ratatui::text::Span::styled(action.command, cmd_style),
                ]));
            }
        }

        let p = Paragraph::new(lines);
        p.render(area, buf);
    }
}
