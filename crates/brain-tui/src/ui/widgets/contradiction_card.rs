//! Side-by-side contradiction card comparing conflicting facts and confidence metrics.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use brain_domain::reflection::StewardshipFinding;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Side-by-side contradiction comparison card widget.
pub struct ContradictionCardWidget<'a> {
    /// Focused stewardship finding.
    pub finding: &'a StewardshipFinding,
}

impl<'a> ContradictionCardWidget<'a> {
    /// Renders contradiction comparison card onto buffer area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = theme.panel(" Finding Details & Evidence ", false);

        let lines = vec![
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("Issue: ", theme.style(ThemeToken::TextSecondary)),
                ratatui::text::Span::styled(
                    &self.finding.summary,
                    theme.style(ThemeToken::Danger).add_modifier(Modifier::BOLD),
                ),
            ]),
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(
                    "Description: ",
                    theme.style(ThemeToken::TextSecondary),
                ),
                ratatui::text::Span::styled(
                    &self.finding.description,
                    theme.style(ThemeToken::TextPrimary),
                ),
            ]),
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("Confidence: ", theme.style(ThemeToken::TextSecondary)),
                ratatui::text::Span::styled(
                    format!("{:.2}", self.finding.confidence.score),
                    theme.style(ThemeToken::Success),
                ),
            ]),
            ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                "Actions: [m] Merge • [a] Archive • [u] Undo/Revert • [i] Inspect Document",
                theme.style(ThemeToken::CodeInline),
            )]),
        ];

        let p = Paragraph::new(lines).block(block);
        p.render(area, buf);
    }
}
