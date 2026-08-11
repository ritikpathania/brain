//! Decoupled Graph Layout Engine computing 2D viewport coordinates for Subgraph vertices.

use brain_domain::graph::{NodeId, Subgraph};
use std::collections::HashMap;

/// 2D coordinate placement of a single graph node on the TUI canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionedNode {
    /// Target node identifier.
    pub id: NodeId,
    /// X coordinate (column index).
    pub x: u16,
    /// Y coordinate (row index).
    pub y: u16,
}

/// Fully positioned graph payload ready for painting on the canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionedGraph {
    /// Computed 2D coordinates for all visible vertices.
    pub positioned_nodes: Vec<PositionedNode>,
    /// Underlying domain Subgraph model.
    pub subgraph: Subgraph,
}

/// Abstract LayoutEngine trait decoupling 2D layout algorithms from widget rendering.
pub trait LayoutEngine {
    /// Computes deterministic 2D coordinates for a Subgraph within viewport boundaries.
    fn compute_layout(&self, subgraph: &Subgraph, viewport: (u16, u16)) -> PositionedGraph;
}

/// 100% deterministic 2D grid layout engine enforcing layout stability.
#[derive(Debug, Clone, Default)]
pub struct DeterministicGridLayoutEngine;

impl DeterministicGridLayoutEngine {
    /// Creates a new DeterministicGridLayoutEngine.
    pub fn new() -> Self {
        Self
    }
}

impl LayoutEngine for DeterministicGridLayoutEngine {
    fn compute_layout(&self, subgraph: &Subgraph, viewport: (u16, u16)) -> PositionedGraph {
        let (width, height) = viewport;
        let mut positioned_nodes = Vec::new();
        let mut existing_coords: HashMap<NodeId, (u16, u16)> = HashMap::new();

        let cols = if width > 40 { 3 } else { 2 };
        let cell_width = width.saturating_div(cols).max(18);
        let cell_height = 4;

        for (idx, node) in subgraph.nodes.iter().enumerate() {
            let row = (idx as u16) / cols;
            let col = (idx as u16) % cols;

            let x = (col * cell_width + 2).min(width.saturating_sub(15));
            let y = (row * cell_height + 2).min(height.saturating_sub(3));

            existing_coords.insert(node.id, (x, y));
            positioned_nodes.push(PositionedNode { id: node.id, x, y });
        }

        PositionedGraph {
            positioned_nodes,
            subgraph: subgraph.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brain_domain::graph::{NodeAggregate, NodeKind};

    #[test]
    fn test_deterministic_layout_stability() {
        let engine = DeterministicGridLayoutEngine::new();
        let seed = NodeId::new();
        let mut subgraph = Subgraph::new(vec![seed]);

        subgraph.add_node(NodeAggregate::new(seed, "Seed", NodeKind::Entity));

        let res1 = engine.compute_layout(&subgraph, (80, 24));
        let res2 = engine.compute_layout(&subgraph, (80, 24));

        assert_eq!(res1, res2);
        assert_eq!(res1.positioned_nodes.len(), 1);
        assert_eq!(res1.positioned_nodes[0].x, 2);
        assert_eq!(res1.positioned_nodes[0].y, 2);
    }
}
