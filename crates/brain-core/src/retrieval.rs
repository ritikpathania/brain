use crate::errors::BrainError;
use crate::graph::RelationshipExpansionDTO;
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
    /// When true, the pipeline attaches `RetrievalExplanation` metadata to the
    /// response. Disabled by default to avoid unnecessary overhead.
    pub explain: bool,
    /// Controls how many hops of graph traversal each `MemorySource` performs
    /// after an initial candidate set is identified.
    ///
    /// - `None` — sources use their default depth (currently 1), preserving
    ///   exact v0.7 retrieval behaviour and baseline comparability.
    /// - `Some(0)` — flat retrieval only; no graph expansion at all.
    /// - `Some(n)` — expand up to `n` hops beyond the initial candidates.
    pub graph_depth: Option<usize>,
    /// When true, retrieves the first-order relationships for each retrieved node
    /// and populates the `relationships` field in the response.
    pub expand_relations: bool,
    /// Reference timestamp (Unix epoch seconds) used for temporal decay calculations.
    /// Injected explicitly during testing or benchmarks to guarantee reproducibility.
    pub reference_time: Option<u64>,
}

/// Per-node retrieval score breakdown, emitted when `RetrievalRequest::explain == true`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeExplanation {
    /// Identifier of the node this explanation belongs to.
    pub node_id: NodeId,
    /// Raw BM25 / FTS score from the lexical channel (if available).
    pub lexical_score: Option<f64>,
    /// Cosine similarity from the semantic channel (if available).
    pub semantic_score: Option<f64>,
    /// Combined fusion score (e.g. from RRF).
    pub fusion_score: Option<f64>,
    /// Final rank position (1-based).
    pub rank: usize,
}

/// Diagnostic explanation attached to a retrieval response when `explain == true`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetrievalExplanation {
    /// The fusion strategy that produced the ranking.
    pub fusion_strategy: String,
    /// Per-node score breakdown.
    pub nodes: Vec<NodeExplanation>,
}

/// Represents the final aggregated and ranked retrieval response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetrievalResponse {
    /// The list of retrieved and ranked nodes.
    pub nodes: Vec<Node>,
    /// Populated only when `RetrievalRequest::explain == true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<RetrievalExplanation>,
    /// Populated only when `RetrievalRequest::expand_relations == true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationships: Option<Vec<RelationshipExpansionDTO>>,
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

/// A candidate memory node returned during retrieval candidate generation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Candidate {
    /// The retrieved domain memory node.
    pub node: brain_domain::Node,
    /// Score from lexical search channel (e.g. BM25).
    pub lexical_score: Option<f64>,
    /// Score from semantic search channel (e.g. cosine similarity).
    pub semantic_score: Option<f64>,
    /// Combined score computed by fusion strategies.
    pub fusion_score: Option<f64>,
    /// The final rank assigned.
    pub rank: Option<usize>,
}

/// Trait defining the contract for a ranking strategy that orders candidates.
pub trait Ranker: Send + Sync {
    /// Ranks the input candidates according to the retrieval request context.
    fn rank(
        &self,
        request: &RetrievalRequest,
        candidates: Vec<Candidate>,
    ) -> Result<Vec<Candidate>, BrainError>;
}

/// Trait defining the contract for fusing lexical and semantic candidates.
pub trait FusionStrategy: Send + Sync {
    /// Fuses independent lexical and semantic candidates into a single ranked list.
    fn fuse(
        &self,
        lexical: Vec<Candidate>,
        semantic: Vec<Candidate>,
    ) -> Result<Vec<Candidate>, BrainError>;
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

/// Trait defining the contract for converting user queries into embedding vectors.
pub trait QueryEmbeddingService: Send + Sync {
    /// Generates a vector embedding for the given search query.
    fn embed_query(&self, query: &str) -> Result<Vec<f32>, BrainError>;
}

/// A default implementation of `QueryEmbeddingService` that wraps an `EmbeddingProvider`.
pub struct DefaultQueryEmbeddingService {
    provider: std::sync::Arc<dyn EmbeddingProvider>,
}

impl std::fmt::Debug for DefaultQueryEmbeddingService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultQueryEmbeddingService")
            .field("provider", &self.provider.name())
            .finish()
    }
}

impl DefaultQueryEmbeddingService {
    /// Creates a new `DefaultQueryEmbeddingService`.
    pub fn new(provider: std::sync::Arc<dyn EmbeddingProvider>) -> Self {
        Self { provider }
    }
}

impl QueryEmbeddingService for DefaultQueryEmbeddingService {
    fn embed_query(&self, query: &str) -> Result<Vec<f32>, BrainError> {
        let start = std::time::Instant::now();
        let result = self.provider.embed(query);
        let duration = start.elapsed();
        let success = result.is_ok();
        tracing::info!(
            target: "brain::telemetry::retrieval",
            stage = "embedding",
            duration_ms = duration.as_millis(),
            query_len = query.len(),
            success = success,
            "Retrieval stage completed: embedding"
        );
        result
    }
}

/// Decay model for the temporal reranking stage.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecayModel {
    /// Exponential decay model ($f(dt) = e^{-\lambda dt}$).
    Exponential,
    /// Logarithmic decay model ($f(dt) = 1 / (1 + \alpha \ln(1 + dt))$).
    Logarithmic,
    /// Linear decay model ($f(dt) = \max(0, 1 - dt/W)$).
    Linear,
    /// Uniform decay model ($f(dt) = 1.0$), which preserves original order.
    Uniform,
}

/// Settings governing temporal ranking.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TemporalRankingSettings {
    /// Flag indicating if temporal reranking is active.
    pub enabled: bool,
    /// The decay model function to apply.
    pub model: DecayModel,
    /// Parameter half-life duration in seconds (primarily for Exponential decay).
    pub half_life_seconds: u64,
    /// General scaling factor applied to decay scores.
    pub scaling_factor: f64,
}

/// A request-scoped context object passed to rerankers.
#[derive(Debug, Clone)]
pub struct RerankContext<'a> {
    /// Reference to the active retrieval request.
    pub request: &'a RetrievalRequest,
    /// Reference to the temporal ranking configuration settings.
    pub config: &'a TemporalRankingSettings,
    /// Resolved static reference time (Unix epoch seconds) to calculate elapsed time consistently.
    pub reference_time: u64,
}

/// Trait defining the contract for post-fusion candidate reranking.
///
/// ### Stability Invariants
/// Implementations of `Reranker` MUST be stable:
/// - Never remove candidate nodes from the input list.
/// - Never duplicate candidate nodes.
/// - Only adjust ordering and/or associate scores.
pub trait Reranker: Send + Sync {
    /// Performs the post-fusion candidate reranking.
    fn rerank(
        &self,
        candidates: Vec<Node>,
        context: &RerankContext<'_>,
    ) -> Result<Vec<Node>, BrainError>;
}
