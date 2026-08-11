//! Grouped-priority evolution overview widget displaying proposals by priority tier.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use brain_domain::evolution::{EvolutionPlan, Priority};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Evolution overview list widget grouped by priority tier.
pub struct EvolutionOverviewWidget<'a> {
    /// Active EvolutionPlan domain aggregate.
    pub plan: &'a EvolutionPlan,
    /// Currently selected index.
    pub selected_index: usize,
}

impl<'a> EvolutionOverviewWidget<'a> {
    /// Renders grouped-priority overview onto buffer area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = theme.panel(" Planned Evolution Proposals ", true);

        let lines = if self.plan.proposals.is_empty() {
            vec![ratatui::text::Line::from(ratatui::text::Span::styled(
                "✓ No evolution proposals pending review. Graph is optimal.",
                theme.style(ThemeToken::Success),
            ))]
        } else {
            let mut result_lines = Vec::new();
            let tiers = [
                Priority::Critical,
                Priority::High,
                Priority::Medium,
                Priority::Low,
            ];

            for priority in &tiers {
                let proposals = self.plan.proposals_by_priority(*priority);
                if !proposals.is_empty() {
                    let header_token = match priority {
                        Priority::Critical => ThemeToken::Danger,
                        Priority::High => ThemeToken::Warning,
                        Priority::Medium => ThemeToken::Accent,
                        Priority::Low => ThemeToken::TextMuted,
                    };

                    result_lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            format!("── {:?} Priority ({}) ──", priority, proposals.len()),
                            theme.style(header_token).add_modifier(Modifier::BOLD),
                        ),
                    ]));

                    for prop in proposals {
                        let is_selected = self
                            .plan
                            .proposals
                            .get(self.selected_index)
                            .is_some_and(|p| p.id == prop.id);

                        let style = if is_selected {
                            theme
                                .style(ThemeToken::TextPrimary)
                                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                        } else {
                            theme.style(ThemeToken::TextSecondary)
                        };

                        result_lines.push(ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled(
                                "   • ",
                                theme.style(ThemeToken::TextMuted),
                            ),
                            ratatui::text::Span::styled(&prop.title, style),
                        ]));
                    }
                }
            }

            result_lines
        };

        let p = Paragraph::new(lines).block(block);
        p.render(area, buf);
    }
}
