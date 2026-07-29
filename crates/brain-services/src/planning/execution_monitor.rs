//! Purely event-sourced `ExecutionMonitor` projection and `ExecutionMetricsSnapshot` (Phase 7 Milestone 7.5).
//!
//! ### Architectural Invariants:
//! 1. `ExecutionMonitor` is a pure event-sourced projection, NOT a state authority.
//! 2. Invariant: `ExecutionMetricsSnapshot` is 100% derivable by replaying the append-only `TaskExecutionEvent` log.
//! 3. Replay idempotency: `Replay(events) == Replay(events)`.
//! 4. Event log remains the canonical source of truth; snapshots are cached representations.

use crate::planning::execution_plan::ExecutionPlanId;
use crate::planning::execution_runtime::{ExecutionId, TaskExecutionEvent, TaskExecutionEventKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Immutable metric snapshot derived strictly from an append-only `TaskExecutionEvent` stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionMetricsSnapshot {
    /// Unique snapshot ID.
    pub snapshot_id: Uuid,
    /// Target execution session ID.
    pub execution_id: Option<ExecutionId>,
    /// Source execution plan ID.
    pub execution_plan_id: Option<ExecutionPlanId>,
    /// Total task steps tracked.
    pub total_tasks: usize,
    /// Tasks completed successfully.
    pub completed_tasks: usize,
    /// Tasks failed permanently.
    pub failed_tasks: usize,
    /// Retries executed.
    pub retried_tasks: usize,
    /// Per-stage duration breakdown in milliseconds (stage_index -> duration_ms).
    pub stage_durations_ms: HashMap<usize, u64>,
    /// Calculated throughput in tasks per second.
    pub throughput_tasks_per_sec: f32,
    /// Timestamp of last processed event in milliseconds.
    pub last_event_timestamp_ms: u64,
}

/// Pure event-sourced monitor projecting `TaskExecutionEvent` streams into `ExecutionMetricsSnapshot`.
#[derive(Debug, Clone, Default)]
pub struct ExecutionMonitor;

impl ExecutionMonitor {
    /// Instantiates a new `ExecutionMonitor`.
    pub fn new() -> Self {
        Self
    }

    /// Projects an append-only event stream into an `ExecutionMetricsSnapshot` deterministically.
    pub fn project_events(&self, events: &[TaskExecutionEvent]) -> ExecutionMetricsSnapshot {
        let mut completed_tasks = 0;
        let mut failed_tasks = 0;
        let mut retried_tasks = 0;

        let mut stage_start_times: HashMap<usize, u64> = HashMap::new();
        let mut stage_durations: HashMap<usize, u64> = HashMap::new();

        let mut first_timestamp_ms = None;
        let mut last_timestamp_ms = 0;

        for event in events {
            if first_timestamp_ms.is_none() {
                first_timestamp_ms = Some(event.timestamp_ms);
            }
            last_timestamp_ms = event.timestamp_ms;

            match event.kind {
                TaskExecutionEventKind::TaskCompleted => {
                    completed_tasks += 1;
                }
                TaskExecutionEventKind::TaskFailed => {
                    failed_tasks += 1;
                }
                TaskExecutionEventKind::TaskDispatched => {
                    if event.message.contains("retry") {
                        retried_tasks += 1;
                    }
                }
                TaskExecutionEventKind::StageStarted => {
                    if let Some(idx) = Self::parse_stage_index(&event.message) {
                        stage_start_times.insert(idx, event.timestamp_ms);
                    }
                }
                TaskExecutionEventKind::StageCompleted => {
                    if let Some(idx) = Self::parse_stage_index(&event.message) {
                        if let Some(start_t) = stage_start_times.get(&idx) {
                            let duration = event.timestamp_ms.saturating_sub(*start_t);
                            stage_durations.insert(idx, duration);
                        }
                    }
                }
                _ => {}
            }
        }

        let total_tasks = completed_tasks + failed_tasks;
        let total_duration_sec = match first_timestamp_ms {
            Some(start) => (last_timestamp_ms.saturating_sub(start) as f32) / 1000.0,
            None => 0.0,
        };

        let throughput_tasks_per_sec = if total_duration_sec > 0.0 {
            completed_tasks as f32 / total_duration_sec
        } else {
            0.0
        };

        ExecutionMetricsSnapshot {
            snapshot_id: Uuid::new_v4(),
            execution_id: None,
            execution_plan_id: None,
            total_tasks,
            completed_tasks,
            failed_tasks,
            retried_tasks,
            stage_durations_ms: stage_durations,
            throughput_tasks_per_sec,
            last_event_timestamp_ms: last_timestamp_ms,
        }
    }

    fn parse_stage_index(msg: &str) -> Option<usize> {
        msg.split_whitespace().last()?.parse::<usize>().ok()
    }
}
