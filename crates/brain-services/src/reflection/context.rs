//! Shared immutable context passed to reflection tasks during execution.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Operational metrics tracker for task execution counters and latencies.
#[derive(Debug, Default)]
pub struct TaskMetrics {
    counters: HashMap<String, Arc<AtomicU64>>,
}

impl TaskMetrics {
    /// Creates a new empty `TaskMetrics`.
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
        }
    }

    /// Increments a counter metric by a specified value.
    pub fn increment(&mut self, key: &str, delta: u64) {
        let entry = self
            .counters
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
        entry.fetch_add(delta, Ordering::Relaxed);
    }

    /// Returns the current value of a counter metric.
    pub fn get(&self, key: &str) -> u64 {
        self.counters
            .get(key)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }
}

/// Shared immutable execution context passed to reflection tasks.
#[derive(Clone)]
pub struct TaskReflectionContext {
    /// Identifier or version tag of the current graph snapshot.
    pub graph_snapshot_id: String,
    /// Task execution configuration parameters.
    pub configuration: HashMap<String, String>,
    /// System clock timestamp in milliseconds.
    pub clock_timestamp_ms: u64,
    /// Shared metrics tracker.
    pub metrics: Arc<TaskMetrics>,
    /// Tokio cancellation token to monitor for task cancellation.
    pub cancellation_token: CancellationToken,
}

impl TaskReflectionContext {
    /// Creates a new `TaskReflectionContext` with default metrics and token.
    pub fn new(graph_snapshot_id: impl Into<String>, clock_timestamp_ms: u64) -> Self {
        Self {
            graph_snapshot_id: graph_snapshot_id.into(),
            configuration: HashMap::new(),
            clock_timestamp_ms,
            metrics: Arc::new(TaskMetrics::new()),
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Attaches a custom cancellation token to the context.
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = token;
        self
    }
}
