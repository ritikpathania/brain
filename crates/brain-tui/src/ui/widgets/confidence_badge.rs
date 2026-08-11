//! Confidence badge widget for rendering categorical confidence levels.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use brain_domain::retrieval::{ConfidenceAssessment, ConfidenceLevel};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Renders a ConfidenceBadge representing query confidence.
pub struct ConfidenceBadge<'a> {
    /// Confidence assessment domain model.
    pub assessment: &'a ConfidenceAssessment,
}

impl<'a> ConfidenceBadge<'a> {
    /// Renders confidence badge into target area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let (label, token) = match self.assessment.level {
            ConfidenceLevel::High => ("High Confidence", ThemeToken::Success),
            ConfidenceLevel::Medium => ("Medium Confidence", ThemeToken::Warning),
            ConfidenceLevel::Low => ("Low Confidence", ThemeToken::Danger),
            ConfidenceLevel::Uncertain => ("Uncertain", ThemeToken::TextMuted),
        };

        let style = theme.style(token).add_modifier(Modifier::BOLD);
        let text = format!(" ● {} ({:.2}) ", label, self.assessment.score);
        let p = Paragraph::new(ratatui::text::Line::from(ratatui::text::Span::styled(
            text, style,
        )));
        p.render(area, buf);
    }
}
