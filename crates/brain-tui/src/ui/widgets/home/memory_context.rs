//! Contextual memory overview stage widget.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

/// Contextual memory state overview widget for the Home screen right-side stage.
#[derive(Debug, Clone)]
pub struct MemoryContextWidget {
    /// Number of indexed long-term memories.
    pub indexed_memories: usize,
    /// Number of active reasoning sessions.
    pub active_sessions: usize,
}

impl MemoryContextWidget {
    /// Renders contextual memory state lines into target area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 10 || area.height < 2 {
            return;
        }

        let title_line = Line::from(vec![Span::styled(
            "Context",
            theme
                .style(ThemeToken::HeaderPrimary)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )]);
        let memory_line = Line::from(vec![Span::styled(
            format!("{} memories indexed", self.indexed_memories),
            theme.style(ThemeToken::TextPrimary),
        )]);
        let session_line = Line::from(vec![Span::styled(
            format!("{} active sessions", self.active_sessions),
            theme.style(ThemeToken::TextMuted),
        )]);

        let lines = vec![title_line, memory_line, session_line];
        let p = ratatui::widgets::Paragraph::new(lines);
        p.render(area, buf);
    }
}
