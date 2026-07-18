use serde::{Deserialize, Serialize};

/// Version 1 DTO for runtime status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Status {
    /// Monotonically increasing uptime in seconds.
    pub uptime_secs: u64,
    /// Storage backend identifier (e.g., "sqlite").
    pub storage_backend: String,
    /// Number of active event subscribers.
    pub active_event_subscribers: usize,
    /// Current health state of the engine ("initializing", "healthy", "shuttingdown", "stopped").
    pub health: String,
}

/// Version 1 DTO for runtime metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Metrics {
    /// Observations ingested.
    pub observations_ingested: u64,
    /// Canonicalization successes.
    pub canonicalization_successes: u64,
    /// Canonicalization failures.
    pub canonicalization_failures: u64,
    /// Reflections executed.
    pub reflections_executed: u64,
    /// Projections executed.
    pub projections_executed: u64,
    /// Retrieval queries.
    pub retrieval_queries: u64,
    /// Latency of the last successful ingest in milliseconds.
    pub last_ingest_duration_ms: Option<u64>,
    /// Latency of the last successful projection in milliseconds.
    pub last_projection_duration_ms: Option<u64>,
    /// Cumulative average duration of the canonicalization stage in milliseconds.
    pub avg_canonicalization_duration_ms: Option<u64>,
    /// Cumulative average duration of the reflection stage in milliseconds.
    pub avg_reflection_duration_ms: Option<u64>,
    /// Cumulative average duration of the event dispatch stage in milliseconds.
    pub avg_dispatch_duration_ms: Option<u64>,
}

/// Version 1 DTO for a runtime operational failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Failure {
    /// Operation name (e.g. "ingest").
    pub operation: String,
    /// Error message detail.
    pub error: String,
    /// Wall-clock timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Version 1 DTO for runtime diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostics {
    /// Recent failures list.
    pub recent_failures: Vec<Failure>,
    /// Uptime duration of the last graceful shutdown in milliseconds.
    pub last_shutdown_duration_ms: Option<u64>,
}

/// Version 1 DTO for runtime capability metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capability {
    /// Unique name key (e.g., "storage").
    pub name: String,
    /// Schema/protocol version.
    pub version: u32,
    /// Summary role.
    pub description: String,
    /// Active state ("active", "degraded", "inactive").
    pub state: String,
    /// Active status.
    pub is_enabled: bool,
    /// Experimental status.
    pub is_experimental: bool,
}

/// Version 1 DTO for search results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchSummary {
    /// Unique document identifier.
    pub id: String,
    /// Kind/category of the node.
    pub kind: String,
    /// Title name.
    pub title: String,
    /// Full-text body.
    pub body: String,
    /// Key-value metadata.
    pub metadata: std::collections::BTreeMap<String, String>,
}

/// Version 1 DTO for mapped runtime events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload")]
#[serde(rename_all = "snake_case")]
pub enum Event {
    /// Ephemeral operational progress of a background task.
    TaskProgress {
        /// Task operation identifier.
        operation_id: String,
        /// Causal tracing identifier.
        correlation_id: String,
        /// Current state of execution.
        state: String,
        /// Originating module.
        source: String,
        /// Monotonic sequence counter.
        sequence: u64,
    },
    /// Signal to invalidate projection read models.
    ProjectionInvalidated {
        /// Type of projection to invalidate.
        projection_type: String,
        /// Current monotonic epoch of the engine.
        epoch: u64,
        /// Causal tracing identifier.
        correlation_id: String,
    },
    /// Domain-level relationship mutation.
    RelationshipEvent {
        /// Name of the domain event.
        event_name: String,
        /// Current monotonic epoch of the engine.
        epoch: u64,
        /// Causal tracing identifier.
        correlation_id: String,
    },
    /// Unclassified or fallback event format.
    Unknown {
        /// String representation of the raw event.
        debug_repr: String,
    },
}
