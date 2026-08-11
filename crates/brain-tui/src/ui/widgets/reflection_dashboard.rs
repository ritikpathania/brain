//! Reflection dashboard widget rendering priority metrics and stewardship summaries.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use brain_domain::reflection::StewardshipReport;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Reflection dashboard overview widget.
pub struct ReflectionDashboardWidget<'a> {
    /// Active StewardshipReport domain aggregate.
    pub report: &'a StewardshipReport,
}

impl<'a> ReflectionDashboardWidget<'a> {
    /// Renders dashboard overview onto buffer area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = theme.panel(" Stewardship Summary ", false);

        let total_findings = self.report.findings.len();
        let total_recommendations = self.report.recommendations.len();
        let total_resolutions = self.report.resolutions.len();

        let lines = vec![
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(
                    "Total Issues Observed: ",
                    theme.style(ThemeToken::TextSecondary),
                ),
                ratatui::text::Span::styled(
                    format!("{}", total_findings),
                    theme
                        .style(ThemeToken::Warning)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(
                    "Action Recommendations: ",
                    theme.style(ThemeToken::TextSecondary),
                ),
                ratatui::text::Span::styled(
                    format!("{}", total_recommendations),
                    theme.style(ThemeToken::Accent),
                ),
            ]),
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(
                    "Applied Resolutions: ",
                    theme.style(ThemeToken::TextSecondary),
                ),
                ratatui::text::Span::styled(
                    format!("{}", total_resolutions),
                    theme.style(ThemeToken::Success),
                ),
            ]),
        ];

        let p = Paragraph::new(lines).block(block);
        p.render(area, buf);
    }
}
