//! User-facing memory retrieval structures and explanations.

use serde::{Serialize, Deserialize};
use crate::identifiers::MessageId;

/// Strongly-typed identifier for retrieval result chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RetrievalId(pub u64);

/// Categorical tier indicating importance of retrieved context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetrievalWeight {
    /// Critical context that directly influences the core response.
    Critical,
    /// High importance context with strong relevance.
    High,
    /// Normal context providing supporting details.
    Normal,
}

/// Extensible categories of retrieved resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    /// A source code file or text document.
    File,
    /// A git version control commit.
    GitCommit,
    /// Architecture Decision Record (ADR).
    Adr,
    /// Request for Comments (RFC).
    Rfc,
    /// Official reference documentation.
    Documentation,
    /// Internal memory database entry.
    Memory,
}

/// Dynamic description of retrieved context origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceReference {
    /// Category of the source.
    pub kind: SourceKind,
    /// Location identifier (e.g. file path, commit hash, URL).
    pub location: String,
    /// Optional line number range (start_line, end_line) within the file.
    pub line_range: Option<(usize, usize)>,
}

/// Semantic rating of matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticSimilarity {
    /// Strong semantic matching score.
    High,
    /// Moderate semantic matching score.
    Medium,
    /// Low semantic matching score but still relevant.
    Low,
}

/// High-level explainability details presented to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRetrievalExplanation {
    /// Keywords that matched the user's prompt.
    pub matched_keywords: Vec<String>,
    /// Semantic similarity rating of the matching context.
    pub semantic_similarity: SemanticSimilarity,
    /// Whether the retrieval was prioritized due to recency/chronology.
    pub recency_boost: bool,
    /// Categorical weight tier of the retrieval.
    pub weight: RetrievalWeight,
    /// Provenance origin details.
    pub provenance: ProvenanceReference,
}

/// Context retrieval item attached to assistant messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalInfo {
    /// Unique identifier for this retrieval.
    pub id: RetrievalId,
    /// The message ID this retrieval is associated with.
    pub message_id: MessageId,
    /// Short human-readable title of the retrieved block.
    pub title: String,
    /// Text excerpt representing the retrieved context.
    pub excerpt: String,
    /// User-facing explainability metadata.
    pub explanation: UserRetrievalExplanation,
}

/// Machine-readable progress status tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetrievalStatus {
    /// Context retrieval is currently executing.
    Retrieving,
    /// Context retrieval completed successfully.
    Completed,
    /// Context retrieval failed.
    Failed,
}

/// Separated transport/protocol booking details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalProtocolState {
    /// Unique identifier for this retrieval.
    pub id: RetrievalId,
    /// Current execution state.
    pub status: RetrievalStatus,
    /// Last processed stream sequence number.
    pub last_sequence: u64,
}
