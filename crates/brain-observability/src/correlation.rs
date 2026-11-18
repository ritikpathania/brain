//! Correlation index: maps `CorrelationId` to a list of operation spans.

use crate::timeline::{OperationSpan, OperationTimeline};
use brain_core::events::{CorrelationId, TaskProgress, TaskState};
use std::collections::HashMap;

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

    /// Returns the number of distinct correlation timelines tracked.
    pub fn len(&self) -> usize {
        self.timelines.len()
    }

    /// Returns `true` if the index contains no timelines.
    pub fn is_empty(&self) -> bool {
        self.timelines.is_empty()
    }

    /// Returns total count of spans recorded across all timelines.
    pub fn total_spans(&self) -> usize {
        self.timelines.values().map(|t| t.spans().len()).sum()
    }

    /// Prunes all timelines that have reached a terminal state (Completed, Failed, Cancelled).
    pub fn prune_completed(&mut self) -> usize {
        let initial_count = self.timelines.len();
        self.timelines.retain(|_, timeline| !timeline.is_complete());
        initial_count - self.timelines.len()
    }

    /// Enforces a maximum capacity of timelines by first pruning completed timelines,
    /// and then trimming excess entries if still above the limit.
    pub fn enforce_retention(&mut self, max_timelines: usize) -> usize {
        if self.timelines.len() <= max_timelines {
            return 0;
        }

        let mut pruned = self.prune_completed();

        if self.timelines.len() > max_timelines {
            let excess = self.timelines.len() - max_timelines;
            let keys_to_remove: Vec<CorrelationId> =
                self.timelines.keys().take(excess).cloned().collect();
            for k in keys_to_remove {
                self.timelines.remove(&k);
                pruned += 1;
            }
        }

        pruned
    }
}
