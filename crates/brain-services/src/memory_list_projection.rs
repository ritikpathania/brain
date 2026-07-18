use brain_core::projection::{ProjectionContext, ProjectionQuery, Projector};
use brain_domain::Node;
use serde::{Deserialize, Serialize};

/// Query parameters for filtering MemoryListProjection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryListQuery {
    /// Limit on the number of items returned.
    pub limit: usize,
}
impl ProjectionQuery for MemoryListQuery {}

/// Concrete view containing a sorted collection of memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryListProjection {
    /// Ordered list of active graph nodes.
    pub items: Vec<Node>,
}

/// Projector implementation mapping canonical graph structures to MemoryListProjection.
pub struct MemoryListProjector;

impl Projector<MemoryListProjection, MemoryListQuery> for MemoryListProjector {
    fn project(&self, context: &ProjectionContext<MemoryListQuery>) -> MemoryListProjection {
        let mut nodes: Vec<Node> = context.graph.nodes.values().cloned().collect();
        // Sort deterministically to satisfy the Projection Determinism invariant
        nodes.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string()));

        let limit = context.query.limit;
        if nodes.len() > limit {
            nodes.truncate(limit);
        }

        MemoryListProjection { items: nodes }
    }
}
