//! Generic Document Inspector modal overlay widget for inspecting source files, markdown, and notes.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Generic Document Inspector modal view state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentInspectorState {
    /// Referenced document identifier.
    pub document_id: String,
    /// Relative or absolute source path string.
    pub source_path: String,
    /// Document line content payload.
    pub content: Vec<String>,
    /// Highlight line range (start_line, end_line).
    pub line_range: Option<(usize, usize)>,
    /// Vertical scroll offset.
    pub scroll_offset: usize,
}

impl DocumentInspectorState {
    /// Creates a new DocumentInspectorState sample for display.
    pub fn new(source_path: impl Into<String>, content: Vec<String>) -> Self {
        Self {
            document_id: "doc-sample".to_string(),
            source_path: source_path.into(),
            content,
            line_range: None,
            scroll_offset: 0,
        }
    }
}

/// Generic Document Inspector modal overlay widget.
pub struct DocumentInspectorModal<'a> {
    /// State inspection payload.
    pub state: &'a DocumentInspectorState,
}

impl<'a> DocumentInspectorModal<'a> {
    /// Renders DocumentInspector modal into target area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let title = format!(" Document Inspector — {} ", self.state.source_path);
        let block = theme.panel(&title, true);

        let mut lines = Vec::new();
        let (range_start, range_end) = self
            .state
            .line_range
            .unwrap_or((1, self.state.content.len()));

        for (idx, line_text) in self.state.content.iter().enumerate() {
            let line_num = idx + 1;
            let is_highlighted = line_num >= range_start && line_num <= range_end;

            let line_num_style = if is_highlighted {
                theme.style(ThemeToken::Accent).add_modifier(Modifier::BOLD)
            } else {
                theme.style(ThemeToken::TextMuted)
            };

            let text_style = if is_highlighted {
                theme
                    .style(ThemeToken::TextPrimary)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.style(ThemeToken::TextSecondary)
            };

            lines.push(ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(format!("{:4} │ ", line_num), line_num_style),
                ratatui::text::Span::styled(line_text, text_style),
            ]));
        }

        let p = Paragraph::new(lines).block(block);
        p.render(area, buf);
    }
}
