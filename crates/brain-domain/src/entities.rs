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

    /// Builder method to specify updated_at timestamp.
    pub fn with_updated_at(mut self, updated_at: u64) -> Self {
        self.updated_at = updated_at;
        self
    }

    /// Builder method to specify label on the node.
    pub fn with_label(mut self, label: String) -> Self {
        self.label = label;
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

    /// Strengthens the relationship weight by 0.1, capped at 1.0.
    pub fn strengthen(&mut self) -> Result<crate::events::DomainEvent, crate::errors::DomainError> {
        if self.weight < 0.0 || self.weight > 1.0 {
            return Err(crate::errors::DomainError::InvalidEdgeWeight(self.weight.to_string()));
        }
        self.weight = (self.weight + 0.1).min(1.0);
        self.updated_at = current_unix_timestamp();
        Ok(crate::events::DomainEvent::RelationshipStrengthened {
            source: self.source.to_string(),
            target: self.target.to_string(),
            relation: self.relation.clone(),
            new_weight: self.weight,
        })
    }

    /// Decays the relationship weight exponentially.
    pub fn decay(&mut self, half_life_secs: f64, delta_t_secs: f64) -> Result<(), crate::errors::DomainError> {
        if half_life_secs <= 0.0 {
            return Err(crate::errors::DomainError::InvalidEdgeWeight(format!("half_life_secs={}", half_life_secs)));
        }
        if delta_t_secs < 0.0 {
            return Err(crate::errors::DomainError::InvalidEdgeWeight(format!("delta_t_secs={}", delta_t_secs)));
        }
        let lambda = 2.0f64.ln() / half_life_secs;
        self.weight = self.weight * (-lambda * delta_t_secs).exp();
        self.updated_at = current_unix_timestamp();
        Ok(())
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

    /// Archives the conversation, preventing further modifications.
    pub fn archive(&mut self) -> Result<crate::events::DomainEvent, crate::errors::DomainError> {
        if self.is_archived() {
            return Err(crate::errors::DomainError::ConversationArchived(self.id.to_string()));
        }
        self.metadata.insert("status".to_string(), "archived".to_string());
        Ok(crate::events::DomainEvent::ConversationArchived {
            conversation_id: self.id.to_string(),
        })
    }

    /// Checks if the conversation is archived.
    pub fn is_archived(&self) -> bool {
        self.metadata.get("status").map(|s| s == "archived").unwrap_or(false)
    }

    /// Adds a message to the conversation if not archived.
    pub fn add_message(&mut self, message: Message) -> Result<(), crate::errors::DomainError> {
        if self.is_archived() {
            return Err(crate::errors::DomainError::ConversationArchived(self.id.to_string()));
        }
        self.messages.push(message);
        Ok(())
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

/// Represents a KnowledgeGraph boundary protecting structural and referential invariants.
/// This acts as an aggregate root for in-memory graph operations.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    /// In-memory map of node ID to Node.
    pub nodes: HashMap<NodeId, Node>,
    /// In-memory map of Edge ID to Edge.
    pub edges: HashMap<EdgeId, Edge>,
}

impl KnowledgeGraph {
    /// Creates a new empty `KnowledgeGraph`.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
        }
    }

    /// Adds a node to the knowledge graph.
    pub fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id, node);
    }

    /// Adds a relationship edge to the knowledge graph.
    /// Validates referential integrity: the source and target nodes must exist.
    pub fn add_edge(&mut self, edge: Edge) -> Result<(), crate::errors::DomainError> {
        if !self.nodes.contains_key(&edge.source) {
            return Err(crate::errors::DomainError::MissingSourceNode(edge.source.to_string()));
        }
        if !self.nodes.contains_key(&edge.target) {
            return Err(crate::errors::DomainError::MissingTargetNode(edge.target.to_string()));
        }
        let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.clone());
        if self.edges.contains_key(&edge_id) {
            return Err(crate::errors::DomainError::EdgeAlreadyExists {
                source_node: edge.source.to_string(),
                target_node: edge.target.to_string(),
                relation: edge.relation.clone(),
            });
        }
        self.edges.insert(edge_id, edge);
        Ok(())
    }

    /// Strengthens an existing edge within the graph.
    pub fn strengthen_relationship(
        &mut self,
        source: NodeId,
        target: NodeId,
        relation: String,
    ) -> Result<(), crate::errors::DomainError> {
        let edge_id = EdgeId::new(source, target, relation.clone());
        if let Some(edge) = self.edges.get_mut(&edge_id) {
            edge.strengthen()?;
            Ok(())
        } else {
            Err(crate::errors::DomainError::MissingSourceNode(format!(
                "Edge {} -> {} [{}] not found",
                source, target, relation
            )))
        }
    }
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents an active user session in the brain system.
/// This acts as an aggregate root for session-related data and goal tracking.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// The unique identifier of the session.
    pub id: SessionId,
    /// The targets / goals tracked within this session.
    pub goals: Vec<String>,
    /// Optional metadata associated with the session.
    pub metadata: HashMap<String, String>,
    /// Unix timestamp when the session was last updated.
    pub updated_at: u64,
}

impl Session {
    /// Creates a new `Session`.
    pub fn new(id: SessionId) -> Self {
        Self {
            id,
            goals: Vec::new(),
            metadata: HashMap::new(),
            updated_at: current_unix_timestamp(),
        }
    }

    /// Adds a goal to the session.
    pub fn add_goal(&mut self, goal: String) -> Result<(), crate::errors::DomainError> {
        let trimmed = goal.trim();
        if trimmed.is_empty() {
            return Err(crate::errors::DomainError::DuplicateGoal("Goal cannot be empty".to_string()));
        }
        if self.goals.iter().any(|g| g == trimmed) {
            return Err(crate::errors::DomainError::DuplicateGoal(trimmed.to_string()));
        }
        self.goals.push(trimmed.to_string());
        self.updated_at = current_unix_timestamp();
        Ok(())
    }

    /// Removes a goal from the session.
    pub fn remove_goal(&mut self, goal: &str) -> Result<(), crate::errors::DomainError> {
        let trimmed = goal.trim();
        if let Some(pos) = self.goals.iter().position(|g| g == trimmed) {
            self.goals.remove(pos);
            self.updated_at = current_unix_timestamp();
            Ok(())
        } else {
            Err(crate::errors::DomainError::GoalNotFound(trimmed.to_string()))
        }
    }
}
