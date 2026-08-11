//! Semantic diff viewer widget displaying human-readable graph transformation details.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use brain_domain::evolution::{EvolutionProposal, SemanticChange};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Semantic graph diff viewer widget.
pub struct ProposalDiffWidget<'a> {
    /// Focused evolution proposal.
    pub proposal: &'a EvolutionProposal,
}

impl<'a> ProposalDiffWidget<'a> {
    /// Renders proposal semantic diff onto buffer area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = theme.panel(" Semantic Graph Transformation Diff ", false);

        let mut lines = vec![
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("Proposal: ", theme.style(ThemeToken::TextSecondary)),
                ratatui::text::Span::styled(
                    &self.proposal.title,
                    theme.style(ThemeToken::Accent).add_modifier(Modifier::BOLD),
                ),
            ]),
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("Status: ", theme.style(ThemeToken::TextSecondary)),
                ratatui::text::Span::styled(
                    format!("{:?}", self.proposal.status),
                    theme.style(ThemeToken::Success),
                ),
            ]),
            ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                "Transformations:",
                theme
                    .style(ThemeToken::TextSecondary)
                    .add_modifier(Modifier::UNDERLINED),
            )]),
        ];

        for change in &self.proposal.diff.changes {
            match change {
                SemanticChange::MergedConcepts {
                    canonical,
                    merged,
                    reason,
                } => {
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "  [Merge] ",
                            theme
                                .style(ThemeToken::Warning)
                                .add_modifier(Modifier::BOLD),
                        ),
                        ratatui::text::Span::styled(
                            format!("'{}'", merged),
                            theme.style(ThemeToken::Danger),
                        ),
                        ratatui::text::Span::styled(" ──> ", theme.style(ThemeToken::TextMuted)),
                        ratatui::text::Span::styled(
                            format!("'{}'", canonical),
                            theme.style(ThemeToken::Success),
                        ),
                    ]));
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            format!("          Reason: {}", reason),
                            theme.style(ThemeToken::TextMuted),
                        ),
                    ]));
                }
                SemanticChange::PromotedEntity { label, reason } => {
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "  [Promote] ",
                            theme
                                .style(ThemeToken::Success)
                                .add_modifier(Modifier::BOLD),
                        ),
                        ratatui::text::Span::styled(
                            format!("Concept '{}' to Entity", label),
                            theme.style(ThemeToken::TextPrimary),
                        ),
                    ]));
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            format!("          Reason: {}", reason),
                            theme.style(ThemeToken::TextMuted),
                        ),
                    ]));
                }
                SemanticChange::PrunedRelationship {
                    source,
                    target,
                    relation,
                    reason,
                } => {
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "  [Prune] ",
                            theme.style(ThemeToken::Danger).add_modifier(Modifier::BOLD),
                        ),
                        ratatui::text::Span::styled(
                            format!("{} ──[{}]──> {}", source, relation, target),
                            theme.style(ThemeToken::TextSecondary),
                        ),
                    ]));
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            format!("          Reason: {}", reason),
                            theme.style(ThemeToken::TextMuted),
                        ),
                    ]));
                }
            }
        }

        lines.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("Shortcuts: [a] Approve • [r] Reject • [x] Execute • [u] Rollback • [i] Inspect Document", theme.style(ThemeToken::CodeInline)),
        ]));

        let p = Paragraph::new(lines).block(block);
        p.render(area, buf);
    }
}
