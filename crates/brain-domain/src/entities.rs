use crate::identifiers::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current Unix timestamp in seconds.
fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The role of a chat message sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// Message sent by the human user.
    User,
    /// Message sent by the AI assistant.
    Assistant,
    /// Message containing system prompt instructions.
    System,
}

/// The type classification of a knowledge graph node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    /// A person entity.
    Person,
    /// A project workspace entity.
    Project,
    /// A file path or source code entity.
    File,
    /// A conversation log entity.
    Conversation,
    /// An abstract concept or topic entity.
    Concept,
    /// An extensible custom classification tag.
    #[serde(untagged)]
    Custom(String),
}

/// The lifecycle states of a system extension plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginState {
    /// Plugin located on disk but not yet processed.
    Discovered,
    /// Dependencies specified in the manifest have been resolved.
    DependenciesResolved,
    /// Manifest schema, signatures, and capabilities validated.
    Validated,
    /// Crate assemblies or scripts loaded into the execution context.
    Loaded,
    /// Handlers and settings initialized.
    Initialized,
    /// Dynamic tools and handlers actively running.
    Active,
    /// Temporarily paused or restricted.
    Suspended,
    /// Explicitly disabled.
    Disabled,
    /// Cleaned up and unloaded from runtime memory.
    Unloaded,
}

/// Represents a knowledge graph node containing semantic properties.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// The unique identifier of the node.
    pub id: NodeId,
    /// The primary label/name of the node.
    pub label: String,
    /// The category or type classification of the node.
    pub node_type: NodeType,
    /// Extensible metadata and key-value properties.
    pub properties: HashMap<String, serde_json::Value>,
    /// Unix timestamp when the node was last updated.
    pub updated_at: u64,
}

impl Node {
    /// Creates a new `Node` with default empty properties and current timestamp.
    pub fn new(id: NodeId, label: String, node_type: NodeType) -> Self {
        Self {
            id,
            label,
            node_type,
            properties: HashMap::new(),
            updated_at: current_unix_timestamp(),
        }
    }

    /// Builder method to specify properties on the node.
    pub fn with_properties(mut self, properties: HashMap<String, serde_json::Value>) -> Self {
        self.properties = properties;
        self
    }
}

/// Represents a directed relationship edge between two nodes in the knowledge graph.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// The source node identifier.
    pub source: NodeId,
    /// The target node identifier.
    pub target: NodeId,
    /// The relation label (e.g. "authored", "part_of").
    pub relation: String,
    /// The weight or confidence score of the relationship.
    pub weight: f64,
    /// Unix timestamp when the edge was last updated.
    pub updated_at: u64,
}

impl Edge {
    /// Creates a new `Edge` with current timestamp.
    pub fn new(source: NodeId, target: NodeId, relation: String, weight: f64) -> Self {
        Self {
            source,
            target,
            relation,
            weight,
            updated_at: current_unix_timestamp(),
        }
    }
}

/// Represents a high-dimensional vector embedding associated with a graph node.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// The node identifier associated with this embedding.
    pub node_id: NodeId,
    /// The vector representation.
    pub vector: Vec<f32>,
    /// The length / dimensions of the vector.
    pub dimension: usize,
}

impl Embedding {
    /// Creates a new `Embedding` and calculates its dimension.
    pub fn new(node_id: NodeId, vector: Vec<f32>) -> Self {
        let dimension = vector.len();
        Self {
            node_id,
            vector,
            dimension,
        }
    }
}

/// Represents an individual chat message in a conversation thread.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The unique identifier of the message.
    pub id: MessageId,
    /// The role of the sender ("user" | "assistant" | "system").
    pub role: MessageRole,
    /// The text content of the message.
    pub content: String,
    /// Unix timestamp when the message was created.
    pub timestamp: u64,
}

impl Message {
    /// Creates a new `Message` with the current timestamp.
    pub fn new(id: MessageId, role: MessageRole, content: String) -> Self {
        Self {
            id,
            role,
            content,
            timestamp: current_unix_timestamp(),
        }
    }
}

/// Represents a conversation log history consisting of serial messages.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// The unique identifier of the conversation.
    pub id: ConversationId,
    /// The ordered list of messages in this conversation.
    pub messages: Vec<Message>,
    /// Optional metadata tag maps.
    pub metadata: HashMap<String, String>,
}

impl Conversation {
    /// Creates a new empty `Conversation`.
    pub fn new(id: ConversationId) -> Self {
        Self {
            id,
            messages: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Creates a new empty `Conversation` with a generated conversation ID.
    pub fn new_empty() -> Self {
        Self::new(ConversationId::new())
    }

    /// Builder method to specify messages in the conversation.
    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    /// Builder method to specify metadata on the conversation.
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Represents a planned tool invocation request formulated by a planning agent.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// The call identifier (correlation ID).
    pub call_id: String,
    /// The registered tool name to be invoked.
    pub tool_name: String,
    /// Input arguments for the tool execution.
    pub arguments: HashMap<String, serde_json::Value>,
}

impl ToolCall {
    /// Creates a new `ToolCall`.
    pub fn new(
        call_id: String,
        tool_name: String,
        arguments: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            call_id,
            tool_name,
            arguments,
        }
    }
}
