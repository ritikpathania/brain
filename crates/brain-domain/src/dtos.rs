use serde::{Deserialize, Serialize};

/// Data Transfer Object representing a node, decoupling storage/internal representations from API/UI.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeDTO {
    /// Stringified unique node identifier (standardized UUID).
    pub id: String,
    /// Name or label of the node.
    pub label: String,
    /// Category or type classification.
    pub node_type: String,
    /// Additional JSON-value properties.
    pub attributes: serde_json::Value,
}

impl NodeDTO {
    /// Creates a new `NodeDTO`.
    pub fn new(
        id: String,
        label: String,
        node_type: String,
        attributes: serde_json::Value,
    ) -> Self {
        Self {
            id,
            label,
            node_type,
            attributes,
        }
    }
}

/// Data Transfer Object representing a relationship edge between two nodes.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDTO {
    /// Stringified identifier of the source node.
    pub source: String,
    /// Stringified identifier of the target node.
    pub target: String,
    /// Relation label (e.g. "authored").
    pub relation: String,
    /// Relative weight/confidence score.
    pub weight: f64,
}

impl EdgeDTO {
    /// Creates a new `EdgeDTO`.
    pub fn new(source: String, target: String, relation: String, weight: f64) -> Self {
        Self {
            source,
            target,
            relation,
            weight,
        }
    }
}

/// Data Transfer Object representing a high-dimensional node embedding.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingDTO {
    /// Stringified identifier of the associated node.
    pub node_id: String,
    /// Floating-point vector values.
    pub vector: Vec<f32>,
}

impl EmbeddingDTO {
    /// Creates a new `EmbeddingDTO`.
    pub fn new(node_id: String, vector: Vec<f32>) -> Self {
        Self { node_id, vector }
    }
}

/// Data Transfer Object encapsulating a memory unit including a node and its local edge connections.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDTO {
    /// The target node metadata DTO.
    pub node: NodeDTO,
    /// Directed incoming edges to this node.
    pub incoming_edges: Vec<EdgeDTO>,
    /// Directed outgoing edges from this node.
    pub outgoing_edges: Vec<EdgeDTO>,
}

impl MemoryDTO {
    /// Creates a new `MemoryDTO`.
    pub fn new(node: NodeDTO, incoming_edges: Vec<EdgeDTO>, outgoing_edges: Vec<EdgeDTO>) -> Self {
        Self {
            node,
            incoming_edges,
            outgoing_edges,
        }
    }
}
