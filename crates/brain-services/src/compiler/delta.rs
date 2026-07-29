//! Pure GraphDelta value object for the Knowledge Compiler.

use brain_domain::dtos::{EdgeDTO, NodeDTO};
use serde::{Deserialize, Serialize};

/// Strongly-typed identifier for a node in a GraphDelta.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed identifier for an edge in a GraphDelta.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub String);

impl std::fmt::Display for EdgeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Pure value object capturing calculated graph mutations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GraphDelta {
    /// Newly added nodes.
    pub added_nodes: Vec<NodeDTO>,
    /// Updated nodes.
    pub updated_nodes: Vec<NodeDTO>,
    /// Removed node IDs.
    pub removed_nodes: Vec<NodeId>,
    /// Newly added relationship edges.
    pub added_edges: Vec<EdgeDTO>,
    /// Updated relationship edges.
    pub updated_edges: Vec<EdgeDTO>,
    /// Removed edge IDs.
    pub removed_edges: Vec<EdgeId>,
}

impl GraphDelta {
    /// Creates an empty `GraphDelta`.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Returns `true` if the delta contains no changes.
    pub fn is_empty(&self) -> bool {
        self.added_nodes.is_empty()
            && self.updated_nodes.is_empty()
            && self.removed_nodes.is_empty()
            && self.added_edges.is_empty()
            && self.updated_edges.is_empty()
            && self.removed_edges.is_empty()
    }
}
