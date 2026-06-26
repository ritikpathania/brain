use serde::{Deserialize, Serialize};
use std::fmt;
use ulid::Ulid;
use uuid::Uuid;

/// Strongly-typed identifier for an active user or system session.
/// Wraps a chronological, sortable `ulid::Ulid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Ulid);

impl SessionId {
    /// Generates a new unique `SessionId` using the current timestamp.
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed identifier for an agent execution run.
/// Wraps a chronological, sortable `ulid::Ulid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub Ulid);

impl RunId {
    /// Generates a new unique `RunId` using the current timestamp.
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for RunId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed identifier for a knowledge graph node.
/// Wraps a standard `uuid::Uuid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    /// Generates a new random unique `NodeId` (UUID v4).
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed identifier for a knowledge graph edge.
/// Uniquely identified by its `source` node, `target` node, and the name of the `relation`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId {
    /// The source node identifier.
    pub source: NodeId,
    /// The target node identifier.
    pub target: NodeId,
    /// The relation label (e.g. "knows", "authored").
    pub relation: String,
}

impl EdgeId {
    /// Creates a new `EdgeId` from source, target, and relation string.
    pub fn new(source: NodeId, target: NodeId, relation: String) -> Self {
        Self {
            source,
            target,
            relation,
        }
    }
}

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({} -[{}]-> {})",
            self.source, self.relation, self.target
        )
    }
}

/// Strongly-typed identifier for an extension plugin.
/// Wraps a chronological, sortable `ulid::Ulid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(pub Ulid);

impl PluginId {
    /// Generates a new unique `PluginId` using the current timestamp.
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for PluginId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed identifier for a conversation log thread.
/// Wraps a chronological, sortable `ulid::Ulid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConversationId(pub Ulid);

impl ConversationId {
    /// Generates a new unique `ConversationId` using the current timestamp.
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ConversationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed identifier for an individual conversation message.
/// Wraps a chronological, sortable `ulid::Ulid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub Ulid);

impl MessageId {
    /// Generates a new message identifier using the current timestamp.
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed identifier for an indexed document source.
/// Wraps a chronological, sortable `ulid::Ulid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub Ulid);

impl DocumentId {
    /// Generates a new unique `DocumentId` using the current timestamp.
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
