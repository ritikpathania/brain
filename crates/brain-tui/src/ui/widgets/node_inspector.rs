//! Node Inspector drawer widget inspecting focused graph node metadata and edges.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use brain_domain::graph::{NodeId, Subgraph};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Node Inspector drawer widget.
pub struct NodeInspectorWidget<'a> {
    /// Active Subgraph domain aggregate.
    pub subgraph: &'a Subgraph,
    /// Currently focused node identifier.
    pub focused_node: Option<NodeId>,
}

impl<'a> NodeInspectorWidget<'a> {
    /// Renders Node Inspector drawer into buffer area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = theme.panel(" Node Inspector ", false);

        let lines = if let Some(target_id) = self.focused_node {
            if let Some(node) = self.subgraph.nodes.iter().find(|n| n.id == target_id) {
                let degree = self.subgraph.degree(target_id);
                vec![
                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "Node: ",
                            theme.style(ThemeToken::TextSecondary),
                        ),
                        ratatui::text::Span::styled(
                            &node.label,
                            theme.style(ThemeToken::Accent).add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "Kind: ",
                            theme.style(ThemeToken::TextSecondary),
                        ),
                        ratatui::text::Span::styled(
                            format!("{:?}", node.kind),
                            theme.style(ThemeToken::TextPrimary),
                        ),
                    ]),
                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "Degree: ",
                            theme.style(ThemeToken::TextSecondary),
                        ),
                        ratatui::text::Span::styled(
                            format!("{}", degree),
                            theme.style(ThemeToken::Success),
                        ),
                    ]),
                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "Pinned: ",
                            theme.style(ThemeToken::TextSecondary),
                        ),
                        ratatui::text::Span::styled(
                            if node.is_pinned { "Yes" } else { "No" },
                            theme.style(ThemeToken::TextMuted),
                        ),
                    ]),
                    ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                        "Press [i] to inspect source document",
                        theme.style(ThemeToken::CodeInline),
                    )]),
                ]
            } else {
                vec![ratatui::text::Line::from(ratatui::text::Span::styled(
                    "No node selected",
                    theme.style(ThemeToken::TextMuted),
                ))]
            }
        } else {
            vec![ratatui::text::Line::from(ratatui::text::Span::styled(
                "Select a node to inspect relationships",
                theme.style(ThemeToken::TextMuted),
            ))]
        };

        let p = Paragraph::new(lines).block(block);
        p.render(area, buf);
    }
}
