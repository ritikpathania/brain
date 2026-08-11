//! Integration test suite for Phase B (Knowledge Graph Explorer).

use brain_domain::graph::{
    EdgeAggregate, GraphSelection, NodeAggregate, NodeId, NodeKind, RelationKind, Subgraph,
};
use brain_domain::RelationId;
use brain_tui::layout::graph_layout::{DeterministicGridLayoutEngine, LayoutEngine};
use brain_tui::ui::navigation::modal::Modal;
use brain_tui::ui::navigation::screen::Screen;
use brain_tui::ui::navigation::stack::NavigationStack;
use brain_tui::ui::screens::graph_explorer::{GraphExplorerScreen, GraphExplorerScreenState};
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_deterministic_layout_computation_invariant() {
    let engine = DeterministicGridLayoutEngine::new();
    let seed1 = NodeId::new();
    let seed2 = NodeId::new();

    let mut subgraph = Subgraph::new(vec![seed1]);
    subgraph.add_node(NodeAggregate::new(seed1, "Node A", NodeKind::Entity));
    subgraph.add_node(NodeAggregate::new(seed2, "Node B", NodeKind::Concept));
    subgraph.add_edge(EdgeAggregate::new(
        seed1,
        seed2,
        RelationId::new("REF"),
        RelationKind::References,
    ));

    let pos1 = engine.compute_layout(&subgraph, (100, 30));
    let pos2 = engine.compute_layout(&subgraph, (100, 30));

    assert_eq!(pos1, pos2);
    assert_eq!(pos1.positioned_nodes.len(), 2);
}

#[test]
fn test_layout_position_preservation_on_expansion() {
    let engine = DeterministicGridLayoutEngine::new();
    let n1 = NodeId::new();
    let n2 = NodeId::new();

    let mut initial_subgraph = Subgraph::new(vec![n1]);
    initial_subgraph.add_node(NodeAggregate::new(n1, "Initial Node", NodeKind::Entity));

    let initial_pos = engine.compute_layout(&initial_subgraph, (100, 30));

    let mut expanded_subgraph = initial_subgraph.clone();
    expanded_subgraph.add_node(NodeAggregate::new(n2, "Expanded Node", NodeKind::Concept));

    let expanded_pos = engine.compute_layout(&expanded_subgraph, (100, 30));

    // Initial node coordinates must remain unchanged
    assert_eq!(
        expanded_pos.positioned_nodes[0].x,
        initial_pos.positioned_nodes[0].x
    );
    assert_eq!(
        expanded_pos.positioned_nodes[0].y,
        initial_pos.positioned_nodes[0].y
    );
}

#[test]
fn test_graph_explorer_rendering_wide_and_compact() {
    let seed = NodeId::new();
    let mut subgraph = Subgraph::new(vec![seed]);
    subgraph.add_node(NodeAggregate::new(seed, "SQLite FTS5", NodeKind::Entity));

    let mut selection = GraphSelection::new();
    selection.focus(seed);

    let state = GraphExplorerScreenState {
        subgraph,
        selection,
    };

    let theme = dark_theme();

    // 1. Wide Viewport (120x30)
    let backend_wide = TestBackend::new(120, 30);
    let mut term_wide = Terminal::new(backend_wide).unwrap();
    term_wide
        .draw(|f| {
            let screen = GraphExplorerScreen { state: &state };
            screen.render(Rect::new(0, 0, 120, 30), f.buffer_mut(), theme);
        })
        .unwrap();

    let buf_wide = format!("{:?}", term_wide.backend().buffer());
    assert!(buf_wide.contains("Knowledge Graph Explorer"));
    assert!(buf_wide.contains("Node Inspector"));

    // 2. Compact Viewport (70x20)
    let backend_compact = TestBackend::new(70, 20);
    let mut term_compact = Terminal::new(backend_compact).unwrap();
    term_compact
        .draw(|f| {
            let screen = GraphExplorerScreen { state: &state };
            screen.render(Rect::new(0, 0, 70, 20), f.buffer_mut(), theme);
        })
        .unwrap();

    let buf_compact = format!("{:?}", term_compact.backend().buffer());
    assert!(buf_compact.contains("Knowledge Graph Explorer"));
}

#[test]
fn test_nested_capability_navigation_stack() {
    let mut stack = NavigationStack::new(Screen::Home);

    // Workspace -> Inspector -> Graph -> Document -> Esc -> Graph -> Esc -> Workspace
    stack.push(Screen::Workspace);
    assert_eq!(stack.current(), Screen::Workspace);

    let inspector_modal = Modal::DocumentInspector;
    assert_eq!(inspector_modal.title(), "Document Inspector");

    stack.push(Screen::GraphExplorer);
    assert_eq!(stack.current(), Screen::GraphExplorer);

    // Return via Esc
    assert_eq!(stack.pop(), Some(Screen::GraphExplorer));
    assert_eq!(stack.current(), Screen::Workspace);
}
