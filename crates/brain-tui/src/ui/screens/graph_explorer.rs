//! Knowledge Graph Explorer screen component.

use crate::layout::graph_layout::{DeterministicGridLayoutEngine, LayoutEngine};
use crate::ui::theme::Theme;
use crate::ui::widgets::graph_canvas::GraphCanvasWidget;
use crate::ui::widgets::node_inspector::NodeInspectorWidget;
use brain_domain::graph::{GraphSelection, Subgraph};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// View state aggregate for GraphExplorer screen.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GraphExplorerScreenState {
    /// Active Subgraph aggregate.
    pub subgraph: Subgraph,
    /// Selection and focus state.
    pub selection: GraphSelection,
}

/// Screen component rendering Knowledge Graph Explorer.
pub struct GraphExplorerScreen<'a> {
    /// Screen view state.
    pub state: &'a GraphExplorerScreenState,
}

impl<'a> GraphExplorerScreen<'a> {
    /// Renders screen view into area buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(32)])
            .split(area);

        let canvas_area = chunks[0];
        let inspector_area = chunks[1];

        let engine = DeterministicGridLayoutEngine::new();
        let positioned_graph = engine.compute_layout(
            &self.state.subgraph,
            (canvas_area.width, canvas_area.height),
        );

        let canvas = GraphCanvasWidget {
            graph: &positioned_graph,
            focused_node: self.state.selection.focused,
        };
        canvas.render(canvas_area, buf, theme);

        let inspector = NodeInspectorWidget {
            subgraph: &self.state.subgraph,
            focused_node: self.state.selection.focused,
        };
        inspector.render(inspector_area, buf, theme);
    }
}
