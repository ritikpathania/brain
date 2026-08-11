//! ChecklistProvider returning dynamic task status cards for new and returning users.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget};

/// Individual checklist item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistItem {
    /// Human-readable task label.
    pub label: &'static str,
    /// Completion state.
    pub completed: bool,
}

/// Dynamic ChecklistProvider evaluating user onboarding and daily task progress.
pub struct ChecklistWidget {
    /// List of onboarding checklist items.
    pub items: Vec<ChecklistItem>,
}

impl Default for ChecklistWidget {
    fn default() -> Self {
        Self {
            items: vec![
                ChecklistItem {
                    label: "Connect to Daemon IPC",
                    completed: true,
                },
                ChecklistItem {
                    label: "Create Memory Session",
                    completed: false,
                },
                ChecklistItem {
                    label: "Ask First Question",
                    completed: false,
                },
                ChecklistItem {
                    label: "Inspect Knowledge Graph",
                    completed: false,
                },
            ],
        }
    }
}

impl ChecklistWidget {
    /// Renders the onboarding progress checklist into the buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = theme.panel(" Quick Progress ", true);
        let mut lines = Vec::new();

        for item in &self.items {
            let mark = if item.completed { "[✓] " } else { "[ ] " };
            let style = if item.completed {
                theme.style(ThemeToken::Success)
            } else {
                theme.style(ThemeToken::TextPrimary)
            };
            lines.push(ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(mark, style),
                ratatui::text::Span::styled(item.label, style),
            ]));
        }

        let p = Paragraph::new(lines).block(block);
        p.render(area, buf);
    }
}
