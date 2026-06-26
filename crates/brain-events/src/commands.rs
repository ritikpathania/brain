use brain_core::BrainError;
use brain_domain::{NodeId, PluginId, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Envelope wrapping commands with execution metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEnvelope<C> {
    /// Unique execution identifier for tracking.
    pub id: uuid::Uuid,
    /// Correlation identifier for request tracing.
    pub correlation_id: uuid::Uuid,
    /// Protocol or command version.
    pub version: String,
    /// Unix timestamp when the command was generated (milliseconds).
    pub timestamp_ms: u64,
    /// The actual command payload.
    pub command: C,
}

impl<C> CommandEnvelope<C> {
    /// Creates a new `CommandEnvelope` with a random correlation ID.
    pub fn new(command: C) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            correlation_id: uuid::Uuid::new_v4(),
            version: "1.0".to_string(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            command,
        }
    }

    /// Creates a new `CommandEnvelope` with a specified correlation ID.
    pub fn new_with_correlation(command: C, correlation_id: uuid::Uuid) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            correlation_id,
            version: "1.0".to_string(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            command,
        }
    }
}

/// Generic command response alias returning a structured BrainError.
pub type CommandResult<T> = Result<T, BrainError>;

/// Input query schema for general memory retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    /// Target session context.
    pub session_id: SessionId,
    /// Raw query text.
    pub query_text: String,
    /// Maximum retrieval results limit.
    pub max_results: usize,
}

impl MemoryQuery {
    /// Creates a new `MemoryQuery`.
    pub fn new(session_id: SessionId, query_text: String, max_results: usize) -> Self {
        Self {
            session_id,
            query_text,
            max_results,
        }
    }
}

/// Input query schema for semantic vector similarity searching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQuery {
    /// Target similarity match vector.
    pub query_vector: Vec<f32>,
    /// Minimum similarity score threshold.
    pub threshold: f32,
    /// Maximum retrieval results limit.
    pub limit: usize,
}

impl SemanticQuery {
    /// Creates a new `SemanticQuery`.
    pub fn new(query_vector: Vec<f32>, threshold: f32, limit: usize) -> Self {
        Self {
            query_vector,
            threshold,
            limit,
        }
    }
}

/// Input query schema for knowledge graph traversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQuery {
    /// Source node to start traversal.
    pub start_node: NodeId,
    /// Maximum traversal depth.
    pub max_depth: u32,
}

impl GraphQuery {
    /// Creates a new `GraphQuery`.
    pub fn new(start_node: NodeId, max_depth: u32) -> Self {
        Self {
            start_node,
            max_depth,
        }
    }
}

/// Input query schema for session details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionQuery {
    /// Target session.
    pub session_id: SessionId,
}

impl SessionQuery {
    /// Creates a new `SessionQuery`.
    pub fn new(session_id: SessionId) -> Self {
        Self { session_id }
    }
}

/// Command sub-enum for session operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionCommand {
    /// Create a new session.
    Create,
    /// Close an active session.
    Close(SessionId),
}

/// Command sub-enum for storage operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageCommand {
    /// Ingest raw text data into a session.
    Ingest {
        /// Target session.
        session_id: SessionId,
        /// Content text to ingest.
        content: String,
    },
    /// Trigger weight decay consolidations on graph edges.
    DecayGraph,
}

/// Command sub-enum for plugin lifecycle controls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginCommand {
    /// Load a plugin from a local file path.
    Load {
        /// Absolute file path.
        path: String,
    },
    /// Unload an active plugin.
    Unload(PluginId),
}

/// Command sub-enum for individual system tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCommand {
    /// Call a tool by name.
    Call {
        /// Registered tool name.
        tool_name: String,
        /// Key-value arguments.
        args: HashMap<String, serde_json::Value>,
    },
}

/// Command sub-enum for settings configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigCommand {
    /// Set a config key to a value.
    Set {
        /// Target setting key.
        key: String,
        /// Setting value.
        value: String,
    },
    /// Reload configurations from files and environment.
    Reload,
}

/// Command sub-enum for executing chat/planning agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentCommand {
    /// Execute an agent run.
    Run {
        /// Target session.
        session_id: SessionId,
        /// Registered agent name.
        agent_name: String,
        /// Prompt input string.
        input: String,
    },
}

/// Parent Command enum wrapping all domain-specific variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// Session operation command.
    Session(SessionCommand),
    /// Storage operation command.
    Storage(StorageCommand),
    /// Plugin operation command.
    Plugin(PluginCommand),
    /// Tool execution command.
    Tool(ToolCommand),
    /// Configuration adjustment command.
    Config(ConfigCommand),
    /// Agent execution command.
    Agent(AgentCommand),
}

/// Trait for dispatching synchronous command envelopes.
pub trait CommandDispatcher: Send + Sync {
    /// Dispatches a command envelope and returns a serialized JSON value response.
    fn dispatch(&self, envelope: CommandEnvelope<Command>) -> CommandResult<serde_json::Value>;
}
