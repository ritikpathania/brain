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

/// Strongly-typed identifier for a session goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GoalId(pub Uuid);

impl GoalId {
    /// Generates a new unique `GoalId` (UUID v4).
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for GoalId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GoalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for GoalId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        std::str::FromStr::from_str(s).map(Self)
    }
}

/// Value object representing a session title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTitle(pub String);

impl Default for SessionTitle {
    fn default() -> Self {
        Self("New Session".to_string())
    }
}

/// Value object representing a domain timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionTimestamp(pub u64);

impl Default for SessionTimestamp {
    fn default() -> Self {
        Self(0)
    }
}

/// Strongly-typed identifier for an agent execution run.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EdgeId {
    /// The source node identifier.
    pub source: NodeId,
    /// The target node identifier.
    pub target: NodeId,
    /// The relation label.
    pub relation: RelationId,
}

impl EdgeId {
    /// Creates a new `EdgeId` from source, target, and relation identifier.
    pub fn new(source: NodeId, target: NodeId, relation: RelationId) -> Self {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
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

impl std::str::FromStr for SessionId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        std::str::FromStr::from_str(s).map(Self)
    }
}

impl std::str::FromStr for RunId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        std::str::FromStr::from_str(s).map(Self)
    }
}

impl std::str::FromStr for NodeId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        std::str::FromStr::from_str(s).map(Self)
    }
}

impl std::str::FromStr for PluginId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(ulid) = ulid::Ulid::from_string(s) {
            Ok(PluginId(ulid))
        } else {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher1 = DefaultHasher::new();
            s.hash(&mut hasher1);
            let h1 = hasher1.finish();

            let mut hasher2 = DefaultHasher::new();
            (s, "salt").hash(&mut hasher2);
            let h2 = hasher2.finish();

            let mut bytes = [0u8; 16];
            bytes[0..8].copy_from_slice(&h1.to_be_bytes());
            bytes[8..16].copy_from_slice(&h2.to_be_bytes());

            Ok(PluginId(ulid::Ulid::from_bytes(bytes)))
        }
    }
}

impl std::str::FromStr for ConversationId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        std::str::FromStr::from_str(s).map(Self)
    }
}

impl std::str::FromStr for MessageId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        std::str::FromStr::from_str(s).map(Self)
    }
}

impl std::str::FromStr for DocumentId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        std::str::FromStr::from_str(s).map(Self)
    }
}

impl<'de> serde::Deserialize<'de> for PluginId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if let Ok(ulid) = ulid::Ulid::from_string(&s) {
            Ok(PluginId(ulid))
        } else {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher1 = DefaultHasher::new();
            s.hash(&mut hasher1);
            let h1 = hasher1.finish();

            let mut hasher2 = DefaultHasher::new();
            (&s, "salt").hash(&mut hasher2);
            let h2 = hasher2.finish();

            let mut bytes = [0u8; 16];
            bytes[0..8].copy_from_slice(&h1.to_be_bytes());
            bytes[8..16].copy_from_slice(&h2.to_be_bytes());

            Ok(PluginId(ulid::Ulid::from_bytes(bytes)))
        }
    }
}

/// Strongly-typed identifier for a graph relationship kind.
/// Wraps a stable string key.
///
/// Does not implement `Default` to prevent constructing invalid empty identifiers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelationId(std::borrow::Cow<'static, str>);

impl RelationId {
    /// Creates a new `RelationId` from a static string slice or owned string.
    pub fn new<S: Into<std::borrow::Cow<'static, str>>>(id: S) -> Self {
        Self(id.into())
    }

    /// Accesses the underlying string identifier as a slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for RelationId {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for RelationId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for RelationId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&'static str> for RelationId {
    fn from(s: &'static str) -> Self {
        Self(std::borrow::Cow::Borrowed(s))
    }
}

impl From<String> for RelationId {
    fn from(s: String) -> Self {
        Self(std::borrow::Cow::Owned(s))
    }
}

impl From<RelationId> for String {
    fn from(id: RelationId) -> Self {
        id.0.into_owned()
    }
}

/// Strongly-typed identifier for an ingestion event.
/// Wraps a standard `uuid::Uuid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventId(pub uuid::Uuid);

impl EventId {
    /// Generates a new random unique `EventId` (UUID v4).
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for EventId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        std::str::FromStr::from_str(s).map(Self)
    }
}

/// Strongly-typed identifier for a workspace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(pub String);

impl WorkspaceId {
    /// Creates a new `WorkspaceId` from a string representation.
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self(id.into())
    }

    /// Accesses the underlying string slice of the WorkspaceId.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for WorkspaceId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

/// Strongly-typed identifier for a client application.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClientId(pub String);

impl ClientId {
    /// Creates a new `ClientId` from a string representation.
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self(id.into())
    }

    /// Accesses the underlying string slice of the ClientId.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ClientId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

/// Strongly-typed identifier for an integration adapter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AdapterId(pub String);

impl AdapterId {
    /// Creates a new `AdapterId` from a string representation.
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self(id.into())
    }

    /// Accesses the underlying string slice of the AdapterId.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AdapterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AdapterId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

/// Strongly-typed identifier for an individual message's timestamp.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct MessageTimestamp(pub u64);

impl fmt::Display for MessageTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed identifier for a search document in the full-text search index.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SearchDocumentId(pub String);

impl SearchDocumentId {
    /// Creates a new SearchDocumentId from a string.
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self(id.into())
    }

    /// Accesses the underlying string slice of the SearchDocumentId.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SearchDocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SearchDocumentId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

/// Strongly-typed transaction epoch identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EpochId(pub u64);

impl EpochId {
    /// Generates the initial epoch (0).
    pub fn initial() -> Self {
        Self(0)
    }

    /// Increments the epoch.
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl Default for EpochId {
    fn default() -> Self {
        Self::initial()
    }
}

impl fmt::Display for EpochId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
