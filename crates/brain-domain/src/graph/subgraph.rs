//! In-memory Subgraph domain aggregate representing bounded graph slices.

use super::edge::EdgeAggregate;
use super::node::NodeAggregate;
use crate::identifiers::NodeId;
use serde::{Deserialize, Serialize};

/// Maximum bounded node count per Subgraph neighborhood view.
pub const MAX_NEIGHBORHOOD_NODES: usize = 100;

/// Bounded Subgraph aggregate containing seed nodes, vertices, and edges.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Subgraph {
    /// Seed node identifiers explaining why this neighborhood exists.
    pub seed_nodes: Vec<NodeId>,
    /// Included node aggregates.
    pub nodes: Vec<NodeAggregate>,
    /// Directed edge aggregates.
    pub edges: Vec<EdgeAggregate>,
}

impl Subgraph {
    /// Creates a new empty Subgraph with specified seed nodes.
    pub fn new(seed_nodes: Vec<NodeId>) -> Self {
        Self {
            seed_nodes,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Adds a node if within capacity bounds and unique.
    pub fn add_node(&mut self, node: NodeAggregate) -> bool {
        if self.nodes.len() >= MAX_NEIGHBORHOOD_NODES {
            return false;
        }
        if !self.nodes.iter().any(|n| n.id == node.id) {
            self.nodes.push(node);
            true
        } else {
            false
        }
    }

    /// Adds a directed edge if unique.
    pub fn add_edge(&mut self, edge: EdgeAggregate) -> bool {
        if !self.edges.iter().any(|e| e.id == edge.id) {
            self.edges.push(edge);
            true
        } else {
            false
        }
    }

    /// Dynamically computes the node degree (inbound + outbound incident edges).
    pub fn degree(&self, node_id: NodeId) -> usize {
        self.edges
            .iter()
            .filter(|e| e.source == node_id || e.target == node_id)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::node::NodeKind;
    use crate::graph::relation::RelationKind;
    use crate::identifiers::RelationId;

    #[test]
    fn test_subgraph_dynamic_degree_calculation() {
        let seed = NodeId::new();
        let target = NodeId::new();
        let mut subgraph = Subgraph::new(vec![seed]);

        let node1 = NodeAggregate::new(seed, "Seed Node", NodeKind::Entity);
        let node2 = NodeAggregate::new(target, "Target Node", NodeKind::Concept);

        subgraph.add_node(node1);
        subgraph.add_node(node2);

        let edge = EdgeAggregate::new(
            seed,
            target,
            RelationId::new("REF"),
            RelationKind::References,
        );
        subgraph.add_edge(edge);

        assert_eq!(subgraph.degree(seed), 1);
        assert_eq!(subgraph.degree(target), 1);
    }
}
