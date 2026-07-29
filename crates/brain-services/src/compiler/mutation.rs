//! Mutation Request structures and types for the Knowledge Compiler.

use brain_domain::SessionId;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Strongly-typed identifier for a mutation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MutationId(pub Ulid);

impl MutationId {
    /// Generates a new unique `MutationId`.
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for MutationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MutationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Payload representing raw observation ingestion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationPayload {
    /// Source origin identifier (e.g. "user", "file_watcher", "agent").
    pub source_origin: String,
    /// Raw observation text or JSON content.
    pub content: String,
}

/// Payload representing reflection findings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingPayload {
    /// Reflection rule or analyzer name.
    pub analyzer: String,
    /// Discovered relationship or candidate fact updates.
    pub description: String,
}

/// Payload representing workspace import operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceImportPayload {
    /// Workspace root directory path or identifier.
    pub workspace_id: String,
    /// Primary language or framework hints.
    pub language_hints: Vec<String>,
}

/// Payload representing synchronization updates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncPayload {
    /// Remote or peer sync origin ID.
    pub sync_origin: String,
    /// Monotonic sync sequence number.
    pub sequence: u64,
}

/// Specific variant classification of a mutation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]

pub enum MutationKind {
    /// Raw observation ingestion request.
    Observe(ObservationPayload),
    /// Reflection finding emission request.
    ReflectionFinding(FindingPayload),
    /// Knowledge snapshot restoration request.
    RestoreSnapshot(String),
    /// Workspace import request.
    ImportWorkspace(WorkspaceImportPayload),
    /// Synchronization update request.
    SyncUpdate(SyncPayload),
}

/// Universal input payload envelope for any proposed state transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationRequest {
    /// Unique mutation request identifier for tracking and idempotency.
    pub id: MutationId,
    /// Monotonic UNIX timestamp in milliseconds when the request was initiated.
    pub timestamp_ms: u64,
    /// Active session ID associated with this mutation.
    pub session_id: SessionId,
    /// Mutation payload variant.
    pub kind: MutationKind,
}

impl MutationRequest {
    /// Creates a new `MutationRequest` with a new `MutationId` and current timestamp.
    pub fn new(session_id: SessionId, kind: MutationKind) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            id: MutationId::new(),
            timestamp_ms,
            session_id,
            kind,
        }
    }
}
