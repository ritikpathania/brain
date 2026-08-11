//! Stewardship list widget displaying categorized findings and recommendations.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use brain_domain::reflection::{FindingKind, StewardshipReport};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Stewardship list widget.
pub struct StewardshipListWidget<'a> {
    /// Active StewardshipReport domain aggregate.
    pub report: &'a StewardshipReport,
    /// Currently selected index.
    pub selected_index: usize,
}

impl<'a> StewardshipListWidget<'a> {
    /// Renders stewardship list onto buffer area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = theme.panel(" Stewardship Findings ", true);

        let lines = if self.report.findings.is_empty() {
            vec![ratatui::text::Line::from(ratatui::text::Span::styled(
                "✓ No stewardship issues detected. Knowledge base is healthy.",
                theme.style(ThemeToken::Success),
            ))]
        } else {
            self.report
                .findings
                .iter()
                .enumerate()
                .map(|(idx, finding)| {
                    let is_selected = idx == self.selected_index;
                    let (badge, token) = match finding.kind {
                        FindingKind::Contradiction => ("[Contradiction]", ThemeToken::Danger),
                        FindingKind::Staleness => ("[Stale]", ThemeToken::Warning),
                        FindingKind::Duplication => ("[Duplicate]", ThemeToken::Accent),
                        FindingKind::Incompleteness => ("[Incomplete]", ThemeToken::TextMuted),
                    };

                    let badge_style = theme.style(token).add_modifier(Modifier::BOLD);
                    let title_style = if is_selected {
                        theme
                            .style(ThemeToken::TextPrimary)
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        theme.style(ThemeToken::TextSecondary)
                    };

                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(format!(" {:15} ", badge), badge_style),
                        ratatui::text::Span::styled(&finding.summary, title_style),
                    ])
                })
                .collect()
        };

        let p = Paragraph::new(lines).block(block);
        p.render(area, buf);
    }
}
