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

/// Version 1 DTO for a compiler diagnostic entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticDto {
    /// Severity level ("error", "warning", "info").
    pub level: String,
    /// Diagnostic classification ("conflicting_facts", "ambiguous_identity", "missing_evidence", "low_confidence", "orphan_concept").
    pub kind: String,
    /// Entity ID or scope path associated with the diagnostic.
    pub target: String,
    /// Human-readable explanation.
    pub message: String,
    /// Actionable suggestion for resolution, if any.
    pub suggestion: Option<String>,
}

/// Version 1 DTO for Knowledge Compiler execution results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeCompilationReport {
    /// Unique compilation execution ID (UUID string).
    pub compilation_id: String,
    /// Wall-clock timestamp of compilation in milliseconds.
    pub timestamp_ms: u64,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Total compiler passes executed in deterministic sequence.
    pub passes_executed: usize,
    /// Number of canonical entities compiled in Knowledge IR.
    pub entities_compiled: usize,
    /// Number of canonical facts compiled in Knowledge IR.
    pub facts_compiled: usize,
    /// Deterministically sorted compiler diagnostics emitted during compilation.
    pub diagnostics: Vec<DiagnosticDto>,
    /// Chronological step-by-step compilation log messages.
    pub details: Vec<String>,
}

/// Version 1 DTO for a task execution trace record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskTraceDto {
    /// Unique task identifier string.
    pub id: String,
    /// Task category ("compile", "project", "reflect", "maintain").
    pub kind: String,
    /// Priority level ("critical", "high", "normal", "low").
    pub priority: String,
    /// Execution status string.
    pub status: String,
    /// Wall-clock timestamp in ms when task was created.
    pub created_at_unix_ms: u64,
    /// Time spent waiting in queue in ms.
    pub wait_duration_ms: u64,
    /// Time spent actively executing in ms.
    pub exec_duration_ms: u64,
}

/// Version 1 DTO for a projection sequence lag record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectionLagDto {
    /// Identifier name of the projection.
    pub projection_id: String,
    /// Last sequence number processed by this projection.
    pub last_processed_sequence: u64,
    /// Maximum sequence number present in the event log.
    pub max_event_sequence: u64,
    /// Unprocessed sequence lag count.
    pub lag_sequence_count: u64,
}

/// Version 1 DTO for background runtime orchestrator status and metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrchestratorStatsDto {
    /// Pending task count in queue.
    pub pending_tasks_count: usize,
    /// Cumulative tasks queued.
    pub tasks_queued: u64,
    /// Cumulative tasks completed.
    pub tasks_completed: u64,
    /// Cumulative tasks failed.
    pub tasks_failed: u64,
    /// Cumulative tasks dropped under backpressure.
    pub tasks_dropped: u64,
    /// Wait latency of last task in ms.
    pub last_task_wait_ms: u64,
    /// Execution latency of last task in ms.
    pub last_task_exec_ms: u64,
    /// Details of currently executing task, if any.
    pub current_running_task: Option<TaskTraceDto>,
    /// Recent task trace history list.
    pub task_history: Vec<TaskTraceDto>,
}

/// Version 1 DTO for a unified point-in-time runtime diagnostics snapshot report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeDiagnosticsReport {
    /// Monotonic sequence number generated when snapshot was captured.
    pub snapshot_sequence: u64,
    /// Wall-clock timestamp in ms when snapshot was captured.
    pub snapshot_timestamp_ms: u64,
    /// Overall derived runtime health ("healthy", "degraded", "unhealthy").
    pub health: String,
    /// Optional reason string explaining non-healthy status.
    pub health_reason: Option<String>,
    /// Orchestrator stats and task trace history snapshot.
    pub orchestrator: OrchestratorStatsDto,
    /// Per-projection lag metrics snapshot.
    pub projection_lags: Vec<ProjectionLagDto>,
    /// Reflection engine telemetry snapshot.
    pub reflection: ReflectionStatusReport,
}

/// Version 1 summary DTO representing a concept node in the knowledge catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConceptSummaryDto {
    /// Unique identifier string of the concept.
    pub id: String,
    /// Canonical display label of the concept.
    pub label: String,
    /// Node classification type (e.g. "Person", "System", "Topic").
    pub node_type: String,
    /// Count of relationship edges connected to this concept.
    pub relationships_count: usize,
}

/// Version 1 DTO representing a relationship edge in the graph explorer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationDetailDto {
    /// Unique target concept identifier string.
    pub target_id: String,
    /// Target concept display label.
    pub target_label: String,
    /// Target concept node type.
    pub target_type: String,
    /// Relation type classification (e.g. "works_on", "knows").
    pub relation: String,
    /// Direction classification ("outgoing" or "incoming").
    pub direction: String,
    /// Relationship weight/confidence score between 0.0 and 1.0.
    pub weight: f64,
}

/// Version 1 DTO representing structured provenance origin metadata for a concept.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceDetailDto {
    /// Source origin classification (e.g. "Ingested", "Inferred", "UserAuthored").
    pub source: String,
    /// Originating compiler pass or rule identifier.
    pub compiler_pass: Option<String>,
    /// Physical location reference string (e.g. file path or URL).
    pub location: String,
    /// Unix timestamp in milliseconds when knowledge was established.
    pub timestamp_ms: u64,
    /// Extra key-value metadata annotations.
    pub extra_info: std::collections::BTreeMap<String, String>,
}

/// Version 1 DTO for complete read-only knowledge concept inspection report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConceptDetailReport {
    /// Unique concept identifier string.
    pub id: String,
    /// Canonical display label.
    pub label: String,
    /// Node classification type.
    pub node_type: String,
    /// Key-value property attributes map.
    pub properties: std::collections::BTreeMap<String, String>,
    /// List of directed relationship edges connected to this concept.
    pub relations: Vec<RelationDetailDto>,
    /// Structured origin provenance metadata.
    pub provenance: ProvenanceDetailDto,
}

/// Version 1 DTO for a pass execution timing metric.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PassMetricDto {
    /// Pass static identifier string.
    pub pass_name: String,
    /// Total times this pass has been executed.
    pub executions: u64,
    /// Total duration spent in this pass in milliseconds.
    pub total_duration_ms: u64,
    /// Average duration per execution in milliseconds.
    pub avg_duration_ms: f64,
}

/// Version 1 DTO for background compiler status and telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompilerStatusReport {
    /// Current graph version epoch sequence.
    pub graph_version: u64,
    /// Total compilations executed.
    pub total_compilations: u64,
    /// Total full graph re-compilations executed.
    pub full_compilations: u64,
    /// Total incremental compilations executed.
    pub incremental_compilations: u64,
    /// Total entities compiled across runs.
    pub entities_compiled_total: u64,
    /// Total facts compiled across runs.
    pub facts_compiled_total: u64,
    /// Total diagnostics emitted across runs.
    pub diagnostics_emitted_total: u64,
    /// Duration of last compilation run in milliseconds, if any.
    pub last_compilation_duration_ms: Option<u64>,
    /// Mode of last compilation run ("full" or "incremental"), if any.
    pub last_compilation_mode: Option<String>,
    /// Per-pass timing metrics list.
    pub pass_metrics: Vec<PassMetricDto>,
}

/// Version 1 lightweight DTO for compiler status summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompilerSummaryDto {
    /// Timestamp of last compilation execution in ms, if any.
    pub last_execution_ms: Option<u64>,
    /// Current graph version epoch sequence.
    pub graph_version: u64,
    /// Total canonical entities compiled.
    pub total_entities: usize,
    /// Total canonical facts compiled.
    pub total_facts: usize,
    /// Number of active diagnostics.
    pub active_diagnostics: usize,
    /// Duration of last compilation run in ms.
    pub last_duration_ms: Option<u64>,
}

/// Version 1 DTO for Knowledge IR structural inspection summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompilerIrSummaryDto {
    /// Current graph version epoch sequence.
    pub graph_version: u64,
    /// Number of canonical entities in IR.
    pub canonical_entities_count: usize,
    /// Number of active canonical facts in IR.
    pub canonical_facts_count: usize,
    /// Number of superseded non-canonical facts in IR.
    pub superseded_facts_count: usize,
    /// Number of directed relations in IR.
    pub relations_count: usize,
    /// Top entity classifications with count tuples.
    pub top_entity_kinds: Vec<(String, usize)>,
}
