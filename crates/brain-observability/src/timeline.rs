//! Immutable operation span and append-only timeline for a single correlation context.

use brain_core::events::{CorrelationId, EventSource, OperationId, SemanticStage, TaskState};
use std::time::SystemTime;

/// An immutable snapshot of one task progress emission.
#[derive(Debug, Clone)]
pub struct OperationSpan {
    /// Unique identifier of the operation that emitted this span.
    pub operation_id: OperationId,
    /// Causal tracing identifier linking spans across operations.
    pub correlation_id: CorrelationId,
    /// Subsystem that emitted this span.
    pub source: EventSource,
    /// Optional semantic stage active at the time of emission.
    pub stage: Option<SemanticStage>,
    /// Task state at this point in the state machine.
    pub state: TaskState,
    /// Monotonically increasing sequence number within the operation.
    pub sequence: u64,
    /// Wall-clock timestamp of the emission.
    pub timestamp: SystemTime,
}

/// Append-only in-memory timeline for all spans belonging to one correlation context.
#[derive(Debug, Default)]
pub struct OperationTimeline {
    spans: Vec<OperationSpan>,
}

impl OperationTimeline {
    /// Appends a new span. Spans should arrive in monotonically increasing sequence order.
    pub fn record(&mut self, span: OperationSpan) {
        self.spans.push(span);
    }

    /// Returns all recorded spans in insertion order.
    pub fn spans(&self) -> &[OperationSpan] {
        &self.spans
    }

    /// Returns `true` if the timeline contains a terminal state (Completed, Failed, or Cancelled).
    pub fn is_complete(&self) -> bool {
        self.spans.iter().any(|s| {
            matches!(
                s.state,
                TaskState::Completed | TaskState::Failed(_) | TaskState::Cancelled
            )
        })
    }
}
