//! Context Assembly, Memory Scoping, Ranking, and Snapshotting Contracts.
//!
//! Defines the contracts for scoping memory retrieval, ranking candidates,
//! deterministic context assembly, and immutable context snapshots per generation.

use brain_domain::{NodeId, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::errors::BrainError;

/// Explicit scope for memory storage and retrieval.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryScope {
    /// Memory scoped to a specific conversational session.
    Session(SessionId),
    /// Memory scoped to a specific workspace / corpus identifier.
    Workspace(String),
    /// Global memory shared across all sessions and workspaces.
    Global,
}

impl std::fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Session(id) => write!(f, "session:{}", id),
            Self::Workspace(w) => write!(f, "workspace:{}", w),
            Self::Global => write!(f, "global"),
        }
    }
}

/// Source retrieval channel that produced a memory candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RetrievalChannel {
    /// BM25 / FTS lexical keyword channel.
    Lexical,
    /// Vector cosine / IVF semantic similarity channel.
    Semantic,
    /// Graph expansion and always-on constraint subgraph channel.
    Graph,
    /// Hybrid multi-channel fusion.
    Hybrid,
}

impl std::fmt::Display for RetrievalChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lexical => write!(f, "lexical"),
            Self::Semantic => write!(f, "semantic"),
            Self::Graph => write!(f, "graph"),
            Self::Hybrid => write!(f, "hybrid"),
        }
    }
}

/// A raw retrieval candidate returned from a search channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalCandidate {
    /// Target entity identifier.
    pub node_id: NodeId,
    /// Entity label or title.
    pub label: String,
    /// Textual content excerpt.
    pub content: String,
    /// Channel that produced this candidate.
    pub channel: RetrievalChannel,
    /// Channel-specific raw relevance score.
    pub raw_score: f64,
    /// Creation or last update timestamp in epoch seconds.
    pub timestamp: u64,
    /// Explicit scope of the candidate memory.
    pub scope: MemoryScope,
    /// Extensible metadata.
    pub metadata: HashMap<String, String>,
}

/// A normalized, deduplicated, and ranked memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedMemory {
    /// Target entity identifier.
    pub node_id: NodeId,
    /// Entity label or title.
    pub label: String,
    /// Textual content excerpt.
    pub content: String,
    /// Primary retrieval channel.
    pub channel: RetrievalChannel,
    /// Raw un-decayed score.
    pub raw_score: f64,
    /// Final calibrated score (after temporal decay and fusion).
    pub final_score: f64,
    /// Creation or update timestamp in epoch seconds.
    pub timestamp: u64,
    /// Explicit scope of the memory.
    pub scope: MemoryScope,
    /// Extensible metadata.
    pub metadata: HashMap<String, String>,
}

/// A single memory item included in an assembled context block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssembledMemory {
    /// Target entity identifier.
    pub node_id: NodeId,
    /// Entity label or title.
    pub label: String,
    /// Textual content excerpt.
    pub content: String,
    /// Channel that produced this memory.
    pub channel: RetrievalChannel,
    /// Calibrated relevance score.
    pub score: f64,
    /// Timestamp in epoch seconds.
    pub timestamp: u64,
    /// Scope of the memory.
    pub scope: MemoryScope,
}

/// Provenance metadata summarizing the sources and score distribution of assembled memories.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryProvenance {
    /// Number of recalled memory items.
    pub count: usize,
    /// Distinct source identifiers (node IDs or workspace labels).
    pub sources: Vec<String>,
    /// Retrieval channels involved in this context snapshot.
    pub channels: Vec<RetrievalChannel>,
    /// Minimum relevance score among selected memories.
    pub min_score: f64,
    /// Maximum relevance score among selected memories.
    pub max_score: f64,
    /// Unique immutable retrieval epoch ID for this generation snapshot.
    pub epoch_id: String,
}

/// An immutable context snapshot captured for a specific generation turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    /// Unique retrieval epoch identifier.
    pub epoch_id: String,
    /// Structured list of memory items included in this snapshot.
    pub items: Vec<AssembledMemory>,
    /// Total estimated token count of the serialized context block.
    pub token_count: usize,
    /// Provenance metadata summary.
    pub provenance: MemoryProvenance,
    /// Canonical serialized Markdown/text representation for model prompt injection.
    pub serialized_context: String,
}

/// Request parameters for context retrieval and assembly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAssemblyRequest {
    /// The user prompt / input text to query against memory.
    pub prompt: String,
    /// Active conversational session ID.
    pub session_id: SessionId,
    /// Active workspace identifier (if any).
    pub workspace_id: Option<String>,
    /// Maximum allowable context token budget.
    pub max_tokens: usize,
    /// Maximum allowable memory items to include.
    pub max_items: usize,
    /// Optional fixed reference epoch timestamp for deterministic temporal evaluation.
    pub reference_time: Option<u64>,
}

/// Trait for ranking and deduplicating raw retrieval candidates.
pub trait MemoryRanker: Send + Sync {
    /// Ranks, deduplicates, and applies temporal decay to retrieval candidates.
    ///
    /// Tie-breaking order contract:
    /// 1. `final_score` DESC
    /// 2. `timestamp` DESC
    /// 3. `node_id` ASC (deterministic tie-breaking)
    fn rank_and_deduplicate(
        &self,
        candidates: Vec<RetrievalCandidate>,
        reference_time: u64,
    ) -> Vec<RankedMemory>;
}

/// Trait for assembling ranked memories into an immutable context snapshot.
pub trait ContextAssembler: Send + Sync {
    /// Clamps ranked memories within token & item budget, preserves whole items,
    /// and formats the canonical context representation.
    fn assemble(
        &self,
        ranked: &[RankedMemory],
        max_tokens: usize,
        max_items: usize,
        epoch_id: &str,
    ) -> ContextSnapshot;
}

/// Trait for orchestrating context retrieval, scoping, ranking, and snapshotting.
#[async_trait::async_trait]
pub trait ContextOrchestrator: Send + Sync {
    /// Retrieves, ranks, and captures an immutable context snapshot for a generation turn.
    async fn assemble_context(
        &self,
        request: ContextAssemblyRequest,
    ) -> Result<ContextSnapshot, BrainError>;

    /// Retrieves and ranks raw memory items for explicit query/inspection RPCs.
    async fn retrieve_memories(
        &self,
        request: ContextAssemblyRequest,
    ) -> Result<Vec<RankedMemory>, BrainError>;

    /// Marks a node as deleted/tombstoned for immediate in-flight candidate exclusion.
    async fn mark_node_deleted(&self, _node_id: NodeId) {}

    /// Removes deletion tombstone on a node (used for rollback on storage failure).
    async fn unmark_node_deleted(&self, _node_id: NodeId) {}
}
