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

/// Version 1 DTO for messages in the event stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "msg_type")]
#[serde(rename_all = "snake_case")]
pub enum StreamMessage {
    /// A domain event from the runtime with its WAL sequence.
    Event {
        /// Monotonic sequence ID.
        sequence: u64,
        /// Mapped event payload.
        event: Event,
    },
    /// A control event signaling changes in the stream lifecycle.
    Control {
        /// Control payload details.
        payload: ControlMessage,
    },
}

/// Version 1 DTO for stream lifecycle control messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "control_type")]
#[serde(rename_all = "snake_case")]
pub enum ControlMessage {
    /// Historical catch-up phase has finished, starting live streaming.
    CatchUpCompleted,
    /// Historical request exceeded replay window and has been truncated.
    ReplayTruncated {
        /// Originally requested start sequence.
        requested_start: u64,
        /// Actual start sequence replayed.
        replayed_start: u64,
    },
    /// Subscription connection closed.
    SubscriptionClosed,
    /// Subscription keep-alive heartbeat.
    Heartbeat,
}

/// Version 1 DTO for pagination specifications.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginationSpec {
    /// Maximum number of items to return.
    pub limit: Option<usize>,
    /// Number of items to skip.
    pub offset: Option<usize>,
}

/// Version 1 DTO for search queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchQuery {
    /// The query text to search for.
    pub text: String,
    /// Optional filter by document kinds ("session", "message", etc.).
    pub kinds: Option<Vec<String>>,
    /// Optional pagination specifications.
    pub pagination: Option<PaginationSpec>,
}

/// Version 1 DTO for projection status metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectionStatus {
    /// Unique name of the projection.
    pub name: String,
    /// Schema/logic version of the projection.
    pub version: u32,
    /// Last successfully processed sequence number.
    pub last_sequence: u64,
    /// Current health status ("idle", "active", "rebuilding", "failed").
    pub status: String,
    /// Last error detail, if status is Failed.
    pub last_error: Option<String>,
    /// Epoch timestamp of the last status update in seconds.
    pub updated_at: u64,
}

/// Version 1 DTO for a detected reflection finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReflectionFindingDto {
    /// Finding category ("duplicate", "contradiction", "link_suggestion").
    pub kind: String,
    /// Confidence score between 0.0 and 1.0.
    pub confidence: f64,
    /// Primary target node IDs involved in the finding.
    pub target_ids: Vec<String>,
    /// Narrative description of why the finding was raised.
    pub details: String,
}

/// Version 1 DTO for a planner recommendation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReflectionRecommendationDto {
    /// Originating pass ID ("duplicate_detection", "contradiction", "link_suggestion", "synthesis").
    pub pass_id: String,
    /// Finding category ("duplicate", "contradiction", "link_suggestion").
    pub finding_kind: String,
    /// Confidence score between 0.0 and 1.0.
    pub confidence: f64,
    /// Target node IDs involved.
    pub target_ids: Vec<String>,
    /// Narrative rationale explaining why the action is recommended.
    pub rationale: String,
    /// Summary description of the proposed command.
    pub command: String,
}

/// Version 1 DTO for a skipped finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkippedFindingDto {
    /// Finding category.
    pub finding_kind: String,
    /// Confidence score.
    pub confidence: f64,
    /// Rationale why the finding was skipped (e.g. below confidence threshold).
    pub reasoning: String,
}

/// Version 1 DTO for reflection execution results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReflectionReport {
    /// Execution ID (UUID string).
    pub execution_id: String,
    /// Wall-clock timestamp of execution in milliseconds.
    pub timestamp_ms: u64,
    /// Total duration of the reflection run in milliseconds.
    pub duration_ms: u64,
    /// Number of findings evaluated.
    pub findings_processed: usize,
    /// Number of commands successfully executed.
    pub commands_executed: usize,
    /// Detected findings list.
    pub findings: Vec<ReflectionFindingDto>,
    /// Planned recommendations list.
    pub recommendations: Vec<ReflectionRecommendationDto>,
    /// Formatted log of executed commands.
    pub executed_commands: Vec<String>,
    /// List of skipped findings with reasons.
    pub skipped_findings: Vec<SkippedFindingDto>,
    /// Log messages of operations performed.
    pub details: Vec<String>,
}

/// Version 1 DTO for background reflection scheduler status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReflectionStatusReport {
    /// Whether the background reflection scheduler is enabled.
    pub background_enabled: bool,
    /// Interval in seconds between background reflection ticks.
    pub interval_secs: u64,
    /// Minimum new WAL events required before triggering a cycle.
    pub min_events_trigger: u64,
    /// Maximum concept nodes evaluated per reflection cycle.
    pub max_nodes_per_cycle: usize,
    /// Time budget in milliseconds per cycle.
    pub cycle_time_budget_ms: u64,
    /// Total reflection cycles executed.
    pub reflections_executed: u64,
    /// Total findings detected across cycles.
    pub reflection_findings_count: u64,
    /// Total commands executed across cycles.
    pub reflection_commands_executed: u64,
    /// Total findings skipped across cycles.
    pub reflection_commands_skipped: u64,
    /// Duration of the last reflection run in milliseconds.
    pub last_reflection_duration_ms: Option<u64>,
}

/// Version 1 lightweight DTO for reflection status summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReflectionSummaryDto {
    /// Timestamp of last execution in ms, if any.
    pub last_execution_ms: Option<u64>,
    /// Total findings detected.
    pub total_findings: u64,
    /// Total commands executed.
    pub total_commands_executed: u64,
    /// Duration of last reflection run in ms.
    pub last_duration_ms: Option<u64>,
    /// Scheduler status ("running", "idle", "disabled").
    pub scheduler_state: String,
}
