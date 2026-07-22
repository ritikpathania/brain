use brain_domain::EdgeDTO;

/// Data Transfer Object representing first-order relationship context for a retrieved node.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelationshipExpansionDTO {
    /// Stringified identifier of the focal node.
    pub node_id: String,
    /// Directed incoming edges to this node.
    pub incoming: Vec<EdgeDTO>,
    /// Directed outgoing edges from this node.
    pub outgoing: Vec<EdgeDTO>,
}

impl RelationshipExpansionDTO {
    /// Creates a new `RelationshipExpansionDTO`.
    pub fn new(node_id: String, incoming: Vec<EdgeDTO>, outgoing: Vec<EdgeDTO>) -> Self {
        Self {
            node_id,
            incoming,
            outgoing,
        }
    }
}
