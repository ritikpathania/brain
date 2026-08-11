//! Graph canvas widget painting PositionedGraph nodes and edges onto the TUI buffer.

use crate::layout::graph_layout::PositionedGraph;
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use brain_domain::graph::{NodeId, NodeKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::Widget;

/// Pure Canvas widget rendering a PositionedGraph.
pub struct GraphCanvasWidget<'a> {
    /// Positioned graph layout output.
    pub graph: &'a PositionedGraph,
    /// Currently focused node identifier.
    pub focused_node: Option<NodeId>,
}

impl<'a> GraphCanvasWidget<'a> {
    /// Renders canvas onto buffer area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = theme.panel(" Knowledge Graph Explorer ", true);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 10 || inner.height < 4 {
            return;
        }

        // 1. Draw Edges
        for edge in &self.graph.subgraph.edges {
            let src_node = self
                .graph
                .positioned_nodes
                .iter()
                .find(|n| n.id == edge.source);
            let tgt_node = self
                .graph
                .positioned_nodes
                .iter()
                .find(|n| n.id == edge.target);

            if let (Some(src), Some(tgt)) = (src_node, tgt_node) {
                let start_x = inner.x + src.x.min(inner.width - 1);
                let start_y = inner.y + src.y.min(inner.height - 1);
                let end_x = inner.x + tgt.x.min(inner.width - 1);
                let end_y = inner.y + tgt.y.min(inner.height - 1);

                if start_y == end_y {
                    let min_x = start_x.min(end_x);
                    let max_x = start_x.max(end_x);
                    for x in (min_x + 6)..max_x {
                        if x < inner.x + inner.width {
                            buf.set_string(x, start_y, "─", theme.style(ThemeToken::TextMuted));
                        }
                    }
                }
            }
        }

        // 2. Draw Positioned Nodes
        for pos in &self.graph.positioned_nodes {
            let node = self.graph.subgraph.nodes.iter().find(|n| n.id == pos.id);
            if let Some(n) = node {
                let x = inner.x + pos.x.min(inner.width.saturating_sub(15));
                let y = inner.y + pos.y.min(inner.height.saturating_sub(1));

                let is_focused = self.focused_node == Some(n.id);
                let token = match n.kind {
                    NodeKind::Entity => ThemeToken::Accent,
                    NodeKind::Concept => ThemeToken::Success,
                    NodeKind::Source => ThemeToken::Warning,
                    NodeKind::Memory => ThemeToken::Danger,
                };

                let style = if is_focused {
                    theme
                        .style(token)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else {
                    theme.style(token)
                };

                let glyph = if n.is_pinned { "★" } else { "○" };
                let text = format!(" ({} {}) ", glyph, n.label);
                buf.set_string(x, y, &text, style);
            }
        }
    }
}
