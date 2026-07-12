use brain_domain::{ConversationId, NodeId, PluginId, PluginState, RunId, SessionId};
use serde::{Deserialize, Serialize};

/// Strongly-typed event topics identifying subscription streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventTopic {
    /// App status, configuration shifts, lifecycle signals.
    System,
    /// User/agent session start, close, transitions.
    Session,
    /// LLM executions, planning loops, token generators.
    Agent,
    /// Database node/edge mutations, embedding storage.
    Storage,
    /// Plugin load/unload triggers, capability updates.
    Plugin,
    /// TUI render events, window resizes, keystrokes.
    UI,
    /// Core business domain events.
    Core,
}

/// Metadata envelope wrapping asynchronous event payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Sequence number assigned by the event log database (populated on load/insert).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    /// Unique event identifier for tracing.
    pub event_id: uuid::Uuid,
    /// Correlation identifier for request tracing.
    pub correlation_id: uuid::Uuid,
    /// Unix timestamp when the event was fired (milliseconds).
    pub timestamp_ms: u64,
    /// Protocol or payload envelope version.
    pub version: String,
    /// System or service origin node that published the event.
    pub source: String,
    /// Strongly-typed event payload.
    pub payload: DomainEvent,
}

impl EventEnvelope {
    /// Creates a new `EventEnvelope` wrapping a `DomainEvent` with a random correlation ID.
    pub fn new(source: String, payload: DomainEvent) -> Self {
        Self {
            sequence: None,
            event_id: uuid::Uuid::new_v4(),
            correlation_id: uuid::Uuid::new_v4(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            version: "1.0".to_string(),
            source,
            payload,
        }
    }

    /// Creates a new `EventEnvelope` wrapping a `DomainEvent` with a specified correlation ID.
    pub fn new_with_correlation(
        source: String,
        payload: DomainEvent,
        correlation_id: uuid::Uuid,
    ) -> Self {
        Self {
            sequence: None,
            event_id: uuid::Uuid::new_v4(),
            correlation_id,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            version: "1.0".to_string(),
            source,
            payload,
        }
    }
}

/// System lifecycle signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    /// Application booted successfully.
    AppStarted,
    /// Interruption signal received requesting system exit.
    ShutdownRequested,
    /// Settings configurations updated.
    ConfigReloaded {
        /// Configuration keys that changed.
        keys_changed: Vec<String>,
    },
}

/// Session boundary signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEvent {
    /// Session created.
    SessionCreated(SessionId),
    /// Session closed.
    SessionClosed(SessionId),
    /// Conversation archived.
    ConversationArchived(ConversationId),
}

/// Agent execution lifecycle signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    /// Executing agent started.
    RunStarted {
        /// Crate-unique execution ID.
        run_id: RunId,
        /// Name of the active agent.
        agent_name: String,
    },
    /// A capability tool called.
    ToolCalled {
        /// Execution tracking ID.
        run_id: RunId,
        /// Name of the invoked tool.
        tool_name: String,
        /// Arguments parameter map.
        arguments: serde_json::Value,
    },
    /// Streaming token chunk emitted.
    TokenGenerated {
        /// Execution tracking ID.
        run_id: RunId,
        /// Chunk text content.
        token: String,
    },
    /// Agent pipeline execution finalized.
    RunFinished {
        /// Execution tracking ID.
        run_id: RunId,
        /// Final response string.
        result: String,
    },
}

/// Database mutations signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageEvent {
    /// Node created or modified in long-term storage.
    NodeInserted(NodeId),
    /// Directed edge created or modified.
    EdgeCreated {
        /// Source node.
        source: NodeId,
        /// Target node.
        target: NodeId,
    },
    /// High-dimensional vector embedding saved for a node.
    EmbeddingStored(NodeId),
    /// Relationship strengthened.
    RelationshipStrengthened {
        /// Source node.
        source: NodeId,
        /// Target node.
        target: NodeId,
        /// Relationship label.
        relation: String,
        /// New weight value.
        weight: f64,
    },
    /// Memory nodes merged.
    NodesMerged {
        /// The target node ID that remains.
        target: NodeId,
        /// The node ID that was absorbed/merged.
        merged: NodeId,
    },
}

/// Dynamic extension plugin lifecycle signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginEvent {
    /// Plugin loaded successfully.
    PluginLoaded(PluginId),
    /// Plugin validation or loading failed.
    PluginFailed {
        /// Identifier of the plugin.
        plugin_id: PluginId,
        /// Error message.
        error: String,
    },
    /// Plugin transitioned lifecycle state.
    PluginStateChanged {
        /// Identifier of the plugin.
        plugin_id: PluginId,
        /// Old lifecycle state.
        old_state: PluginState,
        /// New lifecycle state.
        new_state: PluginState,
    },
}

/// UI interactions and display resizing signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UIEvent {
    /// Keyboard key pressed.
    KeyPressed {
        /// Key code or character string.
        key_code: String,
    },
    /// Viewport terminal window resized.
    ViewportResized {
        /// Width in columns.
        width: u16,
        /// Height in rows.
        height: u16,
    },
}

/// Parent event enum wrapping all domain-specific system signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    /// General system status signal.
    System(SystemEvent),
    /// Session boundary signal.
    Session(SessionEvent),
    /// Agent execution pipeline signal.
    Agent(AgentEvent),
    /// Database transaction mutations signal.
    Storage(StorageEvent),
    /// Plugin lifecycle transition signal.
    Plugin(PluginEvent),
    /// User interface and render view signal.
    UI(UIEvent),
    /// Wrapped core business domain events.
    Core(brain_domain::DomainEvent),
}

/// Thread-safe contract for publishing system events.
pub trait EventPublisher: Send + Sync {
    /// Publishes an event envelope to the message bus.
    fn publish(&self, envelope: EventEnvelope);
}

/// Thread-safe contract for subscribing to specific event streams.
pub trait EventSubscriber: Send + Sync {
    /// Registers a closure handler to be executed when events are published on a topic.
    fn subscribe(&self, topic: EventTopic, handler: Box<dyn Fn(EventEnvelope) + Send + Sync>);
}

/// Persistent event log registry for appending and querying sequence-ordered event envelopes.
pub trait EventLog: Send + Sync {
    /// Appends an event to the log, returning its database-assigned sequence number.
    fn append(&self, envelope: &EventEnvelope) -> Result<u64, brain_core::errors::BrainError>;

    /// Reads events from the log starting at a specific sequence ID (inclusive).
    fn read_from(&self, start_sequence: u64, limit: usize) -> Result<Vec<EventEnvelope>, brain_core::errors::BrainError>;

    /// Retrieves the latest sequence number in the log. Returns 0 if the log is empty.
    fn latest_sequence(&self) -> Result<u64, brain_core::errors::BrainError>;
}

/// Canonical event-log sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNumber(pub u64);
