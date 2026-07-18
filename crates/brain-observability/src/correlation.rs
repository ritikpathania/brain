//! Correlation index: maps `CorrelationId` to a list of operation spans.

use std::collections::HashMap;
use brain_core::events::{CorrelationId, TaskProgress, TaskState};
use crate::timeline::{OperationSpan, OperationTimeline};

/// Cross-correlation index accumulating spans from `TaskProgress` events.
///
/// Keyed by `CorrelationId`. Each entry is a timeline of all spans
/// that share that correlation context, across all operations.
#[derive(Debug, Default)]
pub struct CorrelationIndex {
    timelines: HashMap<CorrelationId, OperationTimeline>,
}

impl CorrelationIndex {
    /// Creates an empty `CorrelationIndex`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ingests a `TaskProgress` event, appending its span to the appropriate timeline.
    pub fn ingest(&mut self, progress: &TaskProgress) {
        let stage = match &progress.state {
            TaskState::Progressing { stage, .. } => Some(*stage),
            _ => None,
        };

        let span = OperationSpan {
            operation_id: progress.operation_id,
            correlation_id: progress.correlation_id,
            source: progress.source,
            stage,
            state: progress.state.clone(),
            sequence: progress.sequence,
            timestamp: progress.timestamp,
        };

        self.timelines
            .entry(progress.correlation_id)
            .or_default()
            .record(span);
    }

    /// Returns all spans for a given correlation ID, or `None` if not found.
    pub fn spans_for(&self, corr_id: CorrelationId) -> Option<&[OperationSpan]> {
        self.timelines.get(&corr_id).map(|t| t.spans())
    }

    /// Returns `true` if the timeline for the given correlation ID is complete.
    pub fn is_complete(&self, corr_id: CorrelationId) -> bool {
        self.timelines
            .get(&corr_id)
            .map(|t| t.is_complete())
            .unwrap_or(false)
    }
}
