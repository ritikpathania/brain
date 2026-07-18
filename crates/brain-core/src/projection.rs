use crate::events::CorrelationId;
use brain_domain::{EpochId, KnowledgeGraph};
use serde::{Deserialize, Serialize};

/// Marker trait representing query parameters for a projection.
pub trait ProjectionQuery: Send + Sync + 'static {}

/// A dummy query when no parameters are required.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NoQuery;
impl ProjectionQuery for NoQuery {}

/// Parameterized context containing snapshot details for projection evaluation.
pub struct ProjectionContext<'a, Q: ProjectionQuery> {
    /// Reference to the read-only KnowledgeGraph state.
    pub graph: &'a KnowledgeGraph,
    /// Monotonic epoch state of the graph.
    pub epoch: EpochId,
    /// Custom parameters or filter criteria.
    pub query: &'a Q,
    /// Causal tracing identifier.
    pub correlation_id: CorrelationId,
}

/// Boundary contract for building read-only views of canonical state.
pub trait Projector<P, Q: ProjectionQuery>: Send + Sync + 'static {
    /// Builds the projection from the canonical memory state under a specific context.
    fn project(&self, context: &ProjectionContext<Q>) -> P;
}
