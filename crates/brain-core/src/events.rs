use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

/// Metadata carrying unique context for a stream event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Unique query execution identifier.
    pub execution_id: Uuid,
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// Epoch timestamp when the event was emitted.
    pub timestamp: SystemTime,
}

/// A sequential packet carrying execution progress or output chunks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Contextual metadata.
    pub metadata: EventMetadata,
    /// Stream payload category.
    pub kind: StreamEventKind,
}

/// Categories of streamed payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEventKind {
    /// A single token chunk of generated assistant text response.
    Token(String),
    /// Descriptive human-readable progress updates.
    Progress {
        /// Diagnostic progress text message.
        message: String,
        /// Monotonic completion percentage indicator.
        percentage: Option<f32>,
    },
    /// A transition signaling entering or leaving a pipeline execution stage.
    Stage {
        /// Symbolic name/identifier of the stage.
        name: String,
        /// Status indicator (true: entering, false: leaving).
        active: bool,
    },
    /// Successful completion event returning full response content.
    Finished {
        /// Final response content.
        response: String,
    },
    /// Execution cancelled signal.
    Cancelled,
    /// Execution error diagnostics.
    Error {
        /// Diagnostics error message description.
        message: String,
    },
    /// Request user authorization for a tool call.
    ToolCallRequest {
        /// Unique tool call ID.
        call_id: ToolCallId,
        /// Name / identifier of the tool.
        tool_id: ToolId,
        /// Arguments json payload.
        arguments: String,
        /// True if user approval is required before execution.
        requires_approval: bool,
    },
    /// Incremental progress log updates from a running tool.
    ToolProgress {
        /// Unique tool call ID.
        call_id: ToolCallId,
        /// Monotonic sequence within the tool call lifecycle.
        sequence: u64,
        /// Determinate or indeterminate progress metrics.
        detail: ToolProgressDetail,
        /// Diagnostic progress text message.
        message: String,
    },
    /// Final result of a tool execution.
    ToolCallResult {
        /// Unique tool call ID.
        call_id: ToolCallId,
        /// Output content or error description from the tool.
        result: String,
        /// True if execution failed.
        is_error: bool,
    },
    /// Context retrieval has started.
    RetrievalStarted {
        /// The query text being searched.
        query: String,
    },
    /// A matching chunk retrieved from local or remote knowledge base.
    RetrievalRetrieved {
        /// Detailed user-facing retrieval entry.
        info: brain_domain::bkf::retrieval::RetrievalInfo,
    },
    /// Retrieval phase completed.
    RetrievalCompleted,
    /// Workspace node IDs that materially influenced retrieval or generation for
    /// this query. Emitted by the TUI client when the daemon echoes `context_used`
    /// in `stream_end.metadata`. Rendered as a transient confirmation in the UI.
    WorkspaceContextUsed(Vec<String>),
}

/// Type-safe opaque identifier for a tool execution call.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCallId(pub String);

/// Type-safe opaque identifier for a tool descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId(pub String);

/// Unit category for tool progress completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgressUnit {
    /// Steps completed.
    Steps,
    /// Bytes processed.
    Bytes,
    /// Arbitrary items completed.
    Items,
}

/// Completion details for a tool execution step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolProgressDetail {
    /// Progress is ongoing but has no absolute total bound.
    Indeterminate,
    /// Progress has known completed and total quantities.
    Determinate {
        /// Number of units completed.
        completed: u64,
        /// Total number of units expected.
        total: u64,
        /// The progress unit kind.
        unit: ProgressUnit,
    },
}

/// Strongly-typed identifier for a single execution run of an operation.
pub type OperationId = uuid::Uuid;
/// Strongly-typed identifier tracing the causal trigger of an execution tree.
pub type CorrelationId = uuid::Uuid;

/// Base trait representing any event flowing through the runtime.
pub trait RuntimeEvent: Send + Sync + 'static {
    /// Returns &dyn std::any::Any to enable dynamic type casting in subscribers and tests.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Contract for dispatching runtime events to all active subscribers.
///
/// Implementations are free to use in-memory channels, persisted queues,
/// or distributed transports. The caller has no visibility into the mechanism.
///
/// NOTE: Subscription management (e.g. `subscribe()`) is intentionally *not* on this trait.
/// It is an implementation detail of each dispatcher. Callers that only dispatch events
/// should program against this trait. Callers that also need to subscribe keep a concrete
/// reference alongside.
pub trait RuntimeEventDispatcher: Send + Sync + 'static {
    /// Dispatch an event to all active subscribers.
    fn dispatch(&self, event: std::sync::Arc<dyn RuntimeEvent>);
}

/// Subsystems that can emit events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSource {
    /// Core Domain subsystem.
    Domain,
    /// Projection and views layer.
    Projection,
    /// Cognitive Reflection engine.
    Reflection,
    /// Background Compaction worker.
    Compaction,
    /// Ingestion/observation handler.
    Ingestion,
    /// Client transport adapter.
    Adapter,
    /// Action Scheduler.
    Scheduler,
}

/// High-level semantic stages for knowledge processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticStage {
    /// Operation is registered and waiting to start.
    Queued,
    /// Raw observation ingress and validation.
    Observation,
    /// Entity and relationship extraction.
    Extraction,
    /// Normalization, deduplication, and merging.
    Synthesis,
    /// Graph reflection and clustering compaction.
    Reflection,
    /// Rebuilding and serving projections.
    Projection,
    /// Completing transaction steps.
    Finalizing,
}

/// State machine boundaries for background tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    /// Task has been registered in the coordinator.
    Created,
    /// Task execution has begun.
    Started,
    /// Task is currently running.
    Progressing {
        /// Current semantic stage of the operation.
        stage: SemanticStage,
        /// Optional completed item count.
        completed_items: Option<usize>,
        /// Optional total bounded item count.
        total_items: Option<usize>,
    },
    /// Task completed successfully.
    Completed,
    /// Task aborted due to a failure error message.
    Failed(String),
    /// Task aborted via a cancel request.
    Cancelled,
}

/// Ephemeral operational metadata for long-running processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    /// Unique execution identifier for the task.
    pub operation_id: OperationId,
    /// Causal tracing identifier.
    pub correlation_id: CorrelationId,
    /// Current task execution state boundaries.
    pub state: TaskState,
    /// Source emitter module.
    pub source: EventSource,
    /// Incremental monotonic task sequence number.
    pub sequence: u64,
    /// Event creation timestamp.
    pub timestamp: SystemTime,
}

impl RuntimeEvent for TaskProgress {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Signal to invalidate projection instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionInstanceInvalidatedEvent {
    /// Name identifier of the projection type.
    pub projection_type: String,
    /// Monotonic epoch state of the graph.
    pub epoch: brain_domain::EpochId,
    /// Source emitter module.
    pub source: EventSource,
    /// Causal tracing identifier.
    pub correlation_id: CorrelationId,
}

impl RuntimeEvent for ProjectionInstanceInvalidatedEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Runtime event wrapping a domain-level relationship mutation fact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRelationshipEvent {
    /// The nested domain event payload.
    pub domain_event: brain_domain::events::DomainEvent,
    /// Monotonic epoch state of the graph.
    pub epoch: brain_domain::EpochId,
    /// Causal tracing identifier.
    pub correlation_id: CorrelationId,
    /// Wall-clock timestamp of the emission.
    pub timestamp: SystemTime,
}

impl RuntimeEvent for RuntimeRelationshipEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_json_roundtrip() {
        let meta = EventMetadata {
            execution_id: Uuid::new_v4(),
            sequence: 42,
            timestamp: SystemTime::now(),
        };
        let event = StreamEvent {
            metadata: meta,
            kind: StreamEventKind::Token("Hello".to_string()),
        };
        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: StreamEvent = serde_json::from_str(&serialized).unwrap();
        assert_eq!(event.metadata.sequence, deserialized.metadata.sequence);
        if let StreamEventKind::Token(token) = deserialized.kind {
            assert_eq!(token, "Hello");
        } else {
            panic!("Expected Token variant");
        }
    }
}
