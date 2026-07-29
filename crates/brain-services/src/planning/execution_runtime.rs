//! Task Execution Runtime (`TaskExecutionRuntime`) managing stage progression, task dispatch, state machine transitions, and event logging (Phase 7 Milestone 7.4).
//!
//! ### Architectural Invariants:
//! 1. `TaskExecutionRuntime` operates **ONLY** on compiled `ExecutionPlan` artifacts.
//! 2. Separation of concerns: `TaskExecutionRuntime` coordinates stage progression, events, and state machine; `TaskExecutor` executes individual task steps.
//! 3. Strongly-typed `ExecutionId(pub Uuid)` identity.
//! 4. Strongly-typed `ExecutionFailure` classification.
//! 5. Causal event ordering: `ExecutionStarted` -> `StageStarted` -> `TaskDispatched` -> `TaskCompleted` -> `StageCompleted` -> `ExecutionCompleted`.
//! 6. Terminal states (`Completed`, `Failed`, `Cancelled`) are mutually exclusive.
//! 7. `ExecutionReport` is compiled and immutable after completion.

use crate::planning::execution_plan::{ExecutionPlan, ExecutionPlanId};
use crate::planning::models::{TaskId, TaskStep};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Strongly-typed execution session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub Uuid);

impl std::fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exec_{}", self.0)
    }
}

/// Classification kind for task execution failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionFailureKind {
    /// Task step execution failed.
    TaskFailure,
    /// Stage synchronization barrier failed.
    StageFailure,
    /// Execution was explicitly cancelled.
    Cancellation,
    /// Task execution timed out.
    Timeout,
    /// Internal runtime error.
    InternalError,
}

/// Strongly-typed task execution failure diagnostic details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionFailure {
    /// Failure classification kind.
    pub kind: ExecutionFailureKind,
    /// Optional target task ID.
    pub task_id: Option<TaskId>,
    /// Descriptive diagnostic message.
    pub message: String,
}

impl std::fmt::Display for ExecutionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.task_id {
            Some(id) => write!(f, "[{:?}] Task '{}': {}", self.kind, id, self.message),
            None => write!(f, "[{:?}] {}", self.kind, self.message),
        }
    }
}

impl std::error::Error for ExecutionFailure {}

/// Explicit execution runtime state machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionState {
    /// Execution plan queued and pending start.
    Pending,
    /// Currently executing stage at specified 0-based index.
    StageRunning(usize),
    /// Stage completed at specified 0-based index.
    StageCompleted(usize),
    /// Execution completed successfully.
    Completed,
    /// Execution failed with diagnostic error.
    Failed(ExecutionFailure),
    /// Execution was explicitly cancelled.
    Cancelled,
}

/// Status of an individual task step execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskExecutionStatus {
    /// Task step pending dispatch.
    Pending,
    /// Task step currently running.
    Running,
    /// Task step completed successfully.
    Succeeded,
    /// Task step failed with error.
    Failed(ExecutionFailure),
    /// Task step skipped due to stage failure or cancellation.
    Skipped,
}

/// Audit record for an individual task step execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskExecutionRecord {
    /// Target task ID.
    pub task_id: TaskId,
    /// Current execution status.
    pub status: TaskExecutionStatus,
    /// Dispatch timestamp in milliseconds.
    pub dispatch_time_ms: u64,
    /// Completion timestamp in milliseconds.
    pub completion_time_ms: Option<u64>,
    /// Recorded retry attempts.
    pub retry_count: u32,
}

/// Event kind classification emitted into the append-only `TaskExecutionEvent` log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskExecutionEventKind {
    /// Execution session started.
    ExecutionStarted,
    /// Stage execution started.
    StageStarted,
    /// Task dispatched for execution.
    TaskDispatched,
    /// Task completed successfully.
    TaskCompleted,
    /// Task failed with error.
    TaskFailed,
    /// Stage completed successfully.
    StageCompleted,
    /// Execution session completed successfully.
    ExecutionCompleted,
    /// Execution session failed.
    ExecutionFailed,
    /// Execution session cancelled.
    ExecutionCancelled,
}

/// Single append-only event item tracking execution lifecycle boundaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskExecutionEvent {
    /// Unique event ID.
    pub event_id: Uuid,
    /// Event classification kind.
    pub kind: TaskExecutionEventKind,
    /// Descriptive message text.
    pub message: String,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Final compiled audit report for an execution session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionReport {
    /// Unique execution session ID.
    pub execution_id: ExecutionId,
    /// Source execution plan ID.
    pub execution_plan_id: ExecutionPlanId,
    /// Terminal execution state.
    pub state: ExecutionState,
    /// Audit records for all task steps.
    pub records: Vec<TaskExecutionRecord>,
    /// Append-only event log.
    pub events: Vec<TaskExecutionEvent>,
    /// Total execution duration in milliseconds.
    pub duration_ms: u64,
}

/// Trait implemented by task step execution handlers.
pub trait TaskExecutor: Send + Sync {
    /// Executes a single `TaskStep` and returns `Result<(), ExecutionFailure>`.
    fn execute_task(&self, task: &TaskStep) -> Result<(), ExecutionFailure>;
}

/// Default mock/in-memory task step executor.
#[derive(Debug, Clone, Default)]
pub struct DefaultTaskExecutor;

impl TaskExecutor for DefaultTaskExecutor {
    fn execute_task(&self, _task: &TaskStep) -> Result<(), ExecutionFailure> {
        Ok(())
    }
}

/// Task execution runtime orchestrating stage progression and task dispatch.
pub struct TaskExecutionRuntime {
    executor: Box<dyn TaskExecutor>,
}

impl Default for TaskExecutionRuntime {
    fn default() -> Self {
        Self::new(Box::new(DefaultTaskExecutor))
    }
}

impl TaskExecutionRuntime {
    /// Instantiates a new `TaskExecutionRuntime` with specified `TaskExecutor`.
    pub fn new(executor: Box<dyn TaskExecutor>) -> Self {
        Self { executor }
    }

    /// Executes an immutable `ExecutionPlan` stage by stage, recording state transitions and events.
    pub fn execute_plan(&self, plan: &ExecutionPlan) -> Result<ExecutionReport, ExecutionFailure> {
        self.execute_plan_with_retry(plan, None)
    }

    /// Executes an immutable `ExecutionPlan` stage by stage using an optional `RetryPolicy`.
    pub fn execute_plan_with_retry(
        &self,
        plan: &ExecutionPlan,
        policy: Option<&crate::planning::retry_policy::RetryPolicy>,
    ) -> Result<ExecutionReport, ExecutionFailure> {
        let execution_id = ExecutionId(Uuid::new_v4());
        let start_time_ms = plan.timestamp_ms;

        let mut events = Vec::new();
        let mut records_map = HashMap::new();

        // Log ExecutionStarted
        events.push(TaskExecutionEvent {
            event_id: Uuid::new_v4(),
            kind: TaskExecutionEventKind::ExecutionStarted,
            message: format!("Started execution session '{}'", execution_id),
            timestamp_ms: start_time_ms,
        });

        let mut current_time_ms = start_time_ms;

        for stage in &plan.stages {
            events.push(TaskExecutionEvent {
                event_id: Uuid::new_v4(),
                kind: TaskExecutionEventKind::StageStarted,
                message: format!("Started stage {}", stage.stage_index),
                timestamp_ms: current_time_ms,
            });

            for &task_id in &stage.parallel_tasks {
                let dummy_step = TaskStep {
                    task_id,
                    description: format!("Executing step {}", task_id),
                    required_capabilities: vec![],
                    confidence: 1.0,
                };

                let mut attempt = 1;
                let mut task_succeeded = false;

                while !task_succeeded {
                    let msg = if attempt == 1 {
                        format!("Dispatched task '{}'", task_id)
                    } else {
                        format!("Dispatched task '{}' (retry attempt {})", task_id, attempt)
                    };

                    events.push(TaskExecutionEvent {
                        event_id: Uuid::new_v4(),
                        kind: TaskExecutionEventKind::TaskDispatched,
                        message: msg,
                        timestamp_ms: current_time_ms,
                    });

                    current_time_ms += 10;

                    match self.executor.execute_task(&dummy_step) {
                        Ok(()) => {
                            task_succeeded = true;
                            events.push(TaskExecutionEvent {
                                event_id: Uuid::new_v4(),
                                kind: TaskExecutionEventKind::TaskCompleted,
                                message: format!("Task '{}' completed successfully", task_id),
                                timestamp_ms: current_time_ms,
                            });

                            records_map.insert(
                                task_id,
                                TaskExecutionRecord {
                                    task_id,
                                    status: TaskExecutionStatus::Succeeded,
                                    dispatch_time_ms: current_time_ms - 10,
                                    completion_time_ms: Some(current_time_ms),
                                    retry_count: attempt - 1,
                                },
                            );
                        }
                        Err(fail) => {
                            if let Some(p) = policy {
                                if p.should_retry(&fail, attempt) {
                                    let delay = p.delay_ms(attempt);
                                    current_time_ms += delay;
                                    attempt += 1;
                                    continue;
                                }
                            }

                            // No retry or retries exhausted
                            events.push(TaskExecutionEvent {
                                event_id: Uuid::new_v4(),
                                kind: TaskExecutionEventKind::TaskFailed,
                                message: format!("Task '{}' failed: {}", task_id, fail.message),
                                timestamp_ms: current_time_ms,
                            });

                            records_map.insert(
                                task_id,
                                TaskExecutionRecord {
                                    task_id,
                                    status: TaskExecutionStatus::Failed(fail.clone()),
                                    dispatch_time_ms: current_time_ms - 10,
                                    completion_time_ms: Some(current_time_ms),
                                    retry_count: attempt - 1,
                                },
                            );

                            let state = ExecutionState::Failed(fail.clone());
                            events.push(TaskExecutionEvent {
                                event_id: Uuid::new_v4(),
                                kind: TaskExecutionEventKind::ExecutionFailed,
                                message: format!("Execution session failed: {}", fail.message),
                                timestamp_ms: current_time_ms,
                            });

                            let records = records_map.into_values().collect();
                            return Ok(ExecutionReport {
                                execution_id,
                                execution_plan_id: plan.execution_plan_id,
                                state,
                                records,
                                events,
                                duration_ms: current_time_ms - start_time_ms,
                            });
                        }
                    }
                }
            }

            events.push(TaskExecutionEvent {
                event_id: Uuid::new_v4(),
                kind: TaskExecutionEventKind::StageCompleted,
                message: format!("Completed stage {}", stage.stage_index),
                timestamp_ms: current_time_ms,
            });
        }

        let state = ExecutionState::Completed;
        events.push(TaskExecutionEvent {
            event_id: Uuid::new_v4(),
            kind: TaskExecutionEventKind::ExecutionCompleted,
            message: format!(
                "Execution session '{}' completed successfully",
                execution_id
            ),
            timestamp_ms: current_time_ms,
        });

        let records = records_map.into_values().collect();

        Ok(ExecutionReport {
            execution_id,
            execution_plan_id: plan.execution_plan_id,
            state,
            records,
            events,
            duration_ms: current_time_ms - start_time_ms,
        })
    }
}
