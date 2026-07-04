use crate::errors::BrainError;
use brain_domain::{Conversation, MemoryDTO, Node, Edge, GraphProvenance, GraphVersion, SessionId};

/// Service managing chat session lifecycle, history persistence, and volatile cache synchronization.
pub trait SessionService: Send + Sync {
    /// Creates a new session with a generated unique ID.
    fn create_session(&self) -> Result<SessionId, BrainError>;

    /// Verifies if a session exists in cache or persistence.
    fn session_exists(&self, id: &SessionId) -> Result<bool, BrainError>;

    /// Loads session conversation history, reading from persistence into the volatile cache if necessary.
    fn load_session(&self, id: &SessionId) -> Result<Conversation, BrainError>;

    /// Saves session conversation history, persisting it to DB and updating cache.
    fn save_session(&self, id: &SessionId, history: &Conversation) -> Result<(), BrainError>;

    /// Ingests a node into the session's volatile cache and database.
    fn ingest_node(&self, id: &SessionId, node: Node) -> Result<(), BrainError>;

    /// Deletes a session's history and cleans up its volatile cache.
    fn delete_session(&self, id: &SessionId) -> Result<(), BrainError>;
}

/// Service providing unified retrieval capabilities over volatile caches (STM) and persistent storage (LTM).
pub trait RetrievalService: Send + Sync {
    /// Retrieves relevant memory context for a given query in a session.
    fn retrieve(
        &self,
        session_id: &SessionId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryDTO>, BrainError>;
}

/// Request parameters for the semantic memory extraction process.
#[derive(Debug, Clone)]
pub struct ExtractionRequest {
    /// The raw text sequence to extract memory structures from.
    pub raw_content: String,
    /// Extensible metadata about the context (e.g. conversation, message).
    pub context_metadata: std::collections::HashMap<String, String>,
}

/// Consolidated result of a semantic memory extraction run.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    /// Extracted entities/nodes.
    pub nodes: Vec<Node>,
    /// Extracted relationships/edges.
    pub edges: Vec<Edge>,
    /// Common provenance indicating the source context.
    pub provenance: GraphProvenance,
    /// Active version of the graph protocol at extraction time.
    pub graph_version: GraphVersion,
}

/// Persistence-agnostic boundary contract for memory extraction.
///
/// Invariants:
/// - Implementations must be purely read-oriented (perform no direct repository mutations).
pub trait MemoryExtractor: Send + Sync {
    /// Parses an extraction request into structured nodes and edges.
    fn extract(&self, request: ExtractionRequest) -> Result<ExtractionResult, BrainError>;
}
