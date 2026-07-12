use serde::{Deserialize, Serialize};
use crate::bkf::ids::*;
use crate::bkf::facts::BKFTargetRef;

/// Index reference to a vector embedding stored externally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddingRef {
    /// ID of the embedding record in the vector database.
    pub embedding_id: String,
    /// Ingestion/generation provider.
    pub provider: String,
    /// Model name used for generation.
    pub model: String,
    /// Vector dimensionality.
    pub dimension: usize,
    /// BKF target object linked to this embedding.
    pub target_id: BKFTargetRef,
}

/// Reference to a physical or logical text chunk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkRef {
    /// Unique identifier of the chunk.
    pub id: String,
    /// BKF target object linked to this chunk.
    pub target_id: BKFTargetRef,
    /// Start character offset.
    pub start_offset: usize,
    /// End character offset.
    pub end_offset: usize,
    /// Number of tokens if available.
    pub token_count: Option<usize>,
}

/// A reference to external/internal sources.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Citation {
    /// Citation ID.
    pub id: BkfCitationId,
    /// Unique key or URL of the external source.
    pub source_id: String,
    /// Optional context description.
    pub description: Option<String>,
    /// Locating detail (page, line range, timestamp).
    pub locator: Option<String>,
}

/// An attachment associated with the document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    /// Attachment ID.
    pub id: BkfAttachmentId,
    /// Base filename.
    pub name: String,
    /// MIME content type.
    pub mime_type: String,
    /// Byte size.
    pub size_bytes: u64,
    /// Hash of content (e.g. sha256).
    pub hash: String,
    /// Optional path or storage reference.
    pub path: Option<String>,
}
