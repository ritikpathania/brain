//! Domain representation for knowledge graph nodes.

use crate::identifiers::NodeId;
use serde::{Deserialize, Serialize};

/// Semantic categorization kind of a knowledge graph node.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum NodeKind {
    /// Canonical domain entity or concept.
    #[default]
    Entity,
    /// High-level abstract concept or topic.
    Concept,
    /// Ingested source asset or document.
    Source,
    /// Temporal session memory or conversation context.
    Memory,
}

/// Domain node aggregate representing a vertex in the knowledge graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeAggregate {
    /// Strongly-typed opaque node identifier.
    pub id: NodeId,
    /// Display label for the node.
    pub label: String,
    /// Semantic node categorization.
    pub kind: NodeKind,
    /// User pin flag indicating whether node is pinned into context.
    pub is_pinned: bool,
}

impl NodeAggregate {
    /// Creates a new NodeAggregate.
    pub fn new(id: NodeId, label: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            id,
            label: label.into(),
            kind,
            is_pinned: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_aggregate_construction() {
        let node_id = NodeId::new();
        let node = NodeAggregate::new(node_id, "SQLite Engine", NodeKind::Entity);
        assert_eq!(node.id, node_id);
        assert_eq!(node.label, "SQLite Engine");
        assert_eq!(node.kind, NodeKind::Entity);
        assert!(!node.is_pinned);
    }
}
