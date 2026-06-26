use crate::errors::BrainError;
use brain_domain::{Node, NodeId, SessionId};
use std::collections::HashSet;
use std::time::Instant;

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

/// Trait defining the contract for an embedding provider that generates vector embeddings.
pub trait EmbeddingProvider: Send + Sync {
    /// Returns the name of the embedding provider.
    fn name(&self) -> &'static str;
    /// Generates a vector embedding for the given input text.
    fn embed(&self, text: &str) -> Result<Vec<f32>, BrainError>;
}

/// A placeholder embedding provider that returns a zero vector.
#[derive(Debug, Clone, Default)]
pub struct NoopEmbeddingProvider;

impl EmbeddingProvider for NoopEmbeddingProvider {
    fn name(&self) -> &'static str {
        "noop"
    }

    fn embed(&self, _text: &str) -> Result<Vec<f32>, BrainError> {
        Ok(vec![0.0; 384])
    }
}

/// Trait defining the contract for looking up stored node embeddings.
pub trait EmbeddingLookup: Send + Sync {
    /// Looks up the embedding vector for a given node ID.
    fn lookup(&self, node_id: &brain_domain::NodeId) -> Result<Option<Vec<f32>>, BrainError>;
}

impl<T: crate::repositories::RepositorySet + ?Sized> EmbeddingLookup for T {
    fn lookup(&self, node_id: &brain_domain::NodeId) -> Result<Option<Vec<f32>>, BrainError> {
        self.embeddings()
            .find_by_node_id(node_id)
            .map(|opt| opt.map(|e| e.vector))
    }
}

impl<T: EmbeddingLookup + ?Sized> EmbeddingLookup for std::sync::Arc<T> {
    fn lookup(&self, node_id: &brain_domain::NodeId) -> Result<Option<Vec<f32>>, BrainError> {
        (**self).lookup(node_id)
    }
}
