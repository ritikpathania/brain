use crate::errors::BrainError;
use brain_domain::{Session, Edge, EdgeId, Embedding, Node, NodeId, SessionId};

/// Trait defining atomic CRUD operations for graph Nodes in the database.
///
/// **Invariants & Safety Rules**:
/// - **ID Immutability**: Node IDs are strictly immutable once persisted. Updates must never modify a node's ID.
/// - **No Silent Deletions**: Repository methods must never silently delete metadata or properties on conflicts.
/// - **Provenance Preservation**: Provenance is intrinsic and immutable. Save operations must never modify the existing provenance record on conflict.
/// - **Deterministic Merge**: Conflict resolution (e.g. merging properties) must be completely deterministic.
/// - **Idempotency**: Duplicate writes must be idempotent and perform no additional side effects.
pub trait NodeRepository: Send + Sync {
    /// Saves a single node to the database (insert or update).
    fn save(&self, node: &Node) -> Result<(), BrainError>;
    /// Saves a collection of nodes in a single transaction.
    fn save_batch(&self, nodes: &[Node]) -> Result<(), BrainError>;
    /// Finds a node by its unique identifier.
    fn find_by_id(&self, id: &NodeId) -> Result<Option<Node>, BrainError>;
    /// Deletes a node by its unique identifier.
    fn delete(&self, id: &NodeId) -> Result<(), BrainError>;
    /// Lists all nodes currently in the database.
    fn list_all(&self) -> Result<Vec<Node>, BrainError>;
}

/// Trait defining atomic CRUD operations for graph Edges in the database.
///
/// **Invariants & Safety Rules**:
/// - **ID Immutability**: Edge IDs (source, target, relation) are strictly immutable once persisted.
/// - **No Silent Deletions**: Repository methods must never silently delete metadata or properties.
/// - **Provenance Preservation**: Edge provenance is intrinsic and immutable. Save operations must never modify existing provenance.
/// - **Idempotency**: Duplicate writes must be idempotent.
pub trait EdgeRepository: Send + Sync {
    /// Saves a single edge to the database (insert or update).
    fn save(&self, edge: &Edge) -> Result<(), BrainError>;
    /// Saves a collection of edges in a single transaction.
    fn save_batch(&self, edges: &[Edge]) -> Result<(), BrainError>;
    /// Finds a single edge by its unique composite identifier.
    fn find_by_id(&self, id: &EdgeId) -> Result<Option<Edge>, BrainError>;
    /// Deletes an edge by its composite identifier.
    fn delete(&self, id: &EdgeId) -> Result<(), BrainError>;
    /// Retrieves all edges connected to (incoming or outgoing from) a node.
    fn get_connections(&self, node_id: &NodeId) -> Result<Vec<Edge>, BrainError>;
    /// Lists all edges currently in the database.
    fn list_all(&self) -> Result<Vec<Edge>, BrainError>;
}

/// Trait defining CRUD operations for high-dimensional Node Embeddings.
pub trait EmbeddingRepository: Send + Sync {
    /// Saves a node's vector embedding.
    fn save(&self, embedding: &Embedding) -> Result<(), BrainError>;
    /// Retrieves the vector embedding associated with a node.
    fn find_by_node_id(&self, node_id: &NodeId) -> Result<Option<Embedding>, BrainError>;
    /// Deletes the embedding associated with a node.
    fn delete(&self, node_id: &NodeId) -> Result<(), BrainError>;
    /// Lists all node embeddings.
    fn list_all_embeddings(&self) -> Result<Vec<Embedding>, BrainError>;
    /// Retrieves all embeddings associated with the specified centroid IDs.
    fn find_by_centroids(&self, centroid_ids: &[i32]) -> Result<Vec<Embedding>, BrainError>;
}

/// Trait defining CRUD operations for active and historical sessions.
pub trait SessionRepository: Send + Sync {
    /// Saves session aggregate.
    fn save_session(&self, id: &SessionId, session: &Session) -> Result<(), BrainError>;
    /// Loads session aggregate by its identifier.
    fn load_session(&self, id: &SessionId) -> Result<Option<Session>, BrainError>;
    /// Deletes historical session data.
    fn delete_session(&self, id: &SessionId) -> Result<(), BrainError>;
}

/// Trait defining key-value settings storage operations.
pub trait ConfigRepository: Send + Sync {
    /// Saves a string config value associated with a key.
    fn save_key(&self, key: &str, val: &str) -> Result<(), BrainError>;
    /// Retrieves the config value associated with a key.
    fn get_key(&self, key: &str) -> Result<Option<String>, BrainError>;
}

/// A collection of domain repositories.
pub trait RepositorySet: Send + Sync {
    /// Returns the Node repository.
    fn nodes(&self) -> &dyn NodeRepository;
    /// Returns the Edge repository.
    fn edges(&self) -> &dyn EdgeRepository;
    /// Returns the Embedding repository.
    fn embeddings(&self) -> &dyn EmbeddingRepository;
    /// Returns the Session repository.
    fn sessions(&self) -> &dyn SessionRepository;
    /// Returns the Config repository.
    fn configs(&self) -> &dyn ConfigRepository;
}

/// A Unit of Work transaction interface.
pub trait StorageTransaction: Send + Sync {
    /// Returns the repositories bound to this transaction.
    fn repositories(&self) -> &dyn RepositorySet;
}
