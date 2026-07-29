//! Operational metrics summarizer aggregating execution latencies, retries, and checkpoint counts.

use brain_events::{ReflectionEventEnvelope, ReflectionRuntimeEvent};
use std::collections::HashMap;

/// Aggregated operational metrics summary summarizing reflection runtime behavior over time.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReflectionMetricsSummary {
    /// Latency per task in milliseconds.
    pub task_latencies_ms: HashMap<String, u64>,
    /// Total retry attempts per task.
    pub task_retry_counts: HashMap<String, u32>,
    /// Total stage checkpoints recorded.
    pub checkpoint_count: usize,
    /// Cumulative structural or state changes applied across tasks.
    pub total_changes_applied: usize,
}

impl ReflectionMetricsSummary {
    /// Creates a new empty `ReflectionMetricsSummary`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records metrics from a versioned `ReflectionEventEnvelope`.
    pub fn record_event(&mut self, envelope: &ReflectionEventEnvelope) {
        match &envelope.event {
            ReflectionRuntimeEvent::TaskCompleted {
                task_id,
                duration_ms,
                changes_applied,
                ..
            } => {
                self.task_latencies_ms.insert(task_id.clone(), *duration_ms);
                self.total_changes_applied += changes_applied;
            }
            ReflectionRuntimeEvent::TaskRetried { task_id, .. } => {
                *self.task_retry_counts.entry(task_id.clone()).or_insert(0) += 1;
            }
            ReflectionRuntimeEvent::CheckpointCreated { .. } => {
                self.checkpoint_count += 1;
            }
            _ => {}
        }
    }
}
