use std::collections::HashSet;
use std::time::Instant;
use brain_domain::{Node, NodeId, SessionId};
use crate::errors::BrainError;

/// Represents a request for retrieving relevant nodes from memory.
#[derive(Debug, Clone)]
pub struct RetrievalRequest {
    /// The unique identifier of the active session.
    pub session_id: SessionId,
    /// The search query or input text.
    pub query: String,
    /// The maximum number of nodes to retrieve.
    pub limit: usize,
    /// The set of node identifiers to exclude from retrieval.
    pub exclude_ids: HashSet<NodeId>,
    /// The deadline after which retrieval should timeout or stop.
    pub deadline: Option<Instant>,
}

/// Represents the final aggregated and ranked retrieval response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetrievalResponse {
    /// The list of retrieved and ranked nodes.
    pub nodes: Vec<Node>,
}

/// Metadata associated with a specific memory source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMetadata {
    /// The name of the memory source.
    pub source_name: &'static str,
}

/// Represents the result retrieved from a single memory source.
#[derive(Debug, Clone)]
pub struct MemorySourceResult {
    /// The retrieved nodes.
    pub nodes: Vec<Node>,
    /// The metadata of the source.
    pub metadata: SourceMetadata,
}

/// Policy specifying how retrieved nodes should be hydrated in the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CacheHydrationPolicy {
    /// No cache hydration.
    Never,
    /// Hydrate retrieved nodes on hit.
    OnHit,
    /// Hydrate retrieved nodes and allow future prefetch implementations.
    Eager,
}

/// Trait defining the contract for a memory source that retrieves nodes based on a request.
pub trait MemorySource: Send + Sync {
    /// Retrieves nodes from the memory source according to the request constraints.
    fn retrieve(&self, request: &RetrievalRequest) -> Result<MemorySourceResult, BrainError>;
}

/// Trait defining the contract for a ranking strategy that ranks/orders retrieved nodes.
pub trait RankingStrategy: Send + Sync {
    /// Ranks the input nodes according to the retrieval request context.
    fn rank(&self, request: &RetrievalRequest, nodes: Vec<Node>) -> Result<Vec<Node>, BrainError>;
}

/// A placeholder ranking strategy that returns the input nodes unchanged.
#[derive(Debug, Clone, Default)]
pub struct IdentityRanking;

impl RankingStrategy for IdentityRanking {
    fn rank(&self, _request: &RetrievalRequest, nodes: Vec<Node>) -> Result<Vec<Node>, BrainError> {
        Ok(nodes)
    }
}
