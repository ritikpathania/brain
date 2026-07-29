//! Modular reflection contracts, task traits, and execution reports.

use crate::reconciliation::PassDiagnostic;
use brain_domain::CanonicalEntity;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Classification kind for independently schedulable reflection tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ReflectionTaskKind {
    /// Ingestion reconciliation and contradiction repair.
    Repair,
    /// Lifecycle policy evaluation and state transitions.
    Strengthen,
    /// Vector embedding freshness validation and re-embedding.
    EmbeddingRefresh,
    /// Graph centrality (PageRank, degree) pre-computation.
    Centrality,
    /// Deterministic summary node generation.
    Summarize,
    /// Storage, index, and FTS database optimization.
    Optimize,
}

/// Execution metrics and report produced by a single reflection task.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskReport {
    /// Name of the reflection task.
    pub task_name: &'static str,
    /// Task classification kind.
    pub task_kind: ReflectionTaskKind,
    /// Items inspected during task execution.
    pub items_processed: usize,
    /// Structural or state changes applied.
    pub changes_applied: usize,
    /// Structured diagnostic messages generated.
    pub diagnostics: Vec<PassDiagnostic>,
    /// Task execution duration.
    pub duration: Duration,
}

/// Execution mode for triggering reflection runs.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum ReflectionExecutionMode {
    /// Triggered on daemon startup.
    Startup,
    /// Triggered on periodic timer schedules.
    Periodic,
    /// Triggered during system idle windows.
    Idle,
    /// Triggered manually via CLI or API command.
    #[default]
    Manual,
    /// Full offline "Dream Mode" maintenance orchestration plan.
    Dream,
}

/// Comprehensive report produced by executing a multi-task `ReflectionPlan`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReflectionReport {
    /// Mode used to trigger the reflection run.
    pub execution_mode: ReflectionExecutionMode,
    /// Reports produced by each executed task in plan sequence.
    pub task_reports: Vec<TaskReport>,
    /// Total duration of the reflection run.
    pub total_duration: Duration,
    /// Cumulative structural or state changes applied.
    pub total_changes: usize,
}

/// Strongly-typed globally stable task identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    /// Creates a new `TaskId`.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the string slice representation of the task ID.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Declarative per-task retry policy configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRetryPolicy {
    /// Maximum allowed total attempts.
    pub max_attempts: u32,
    /// Backoff delay in milliseconds between retries.
    pub backoff_ms: u64,
    /// Consecutive failure threshold to trigger circuit breaker.
    pub circuit_breaker_threshold: u32,
}

impl Default for TaskRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_ms: 100,
            circuit_breaker_threshold: 5,
        }
    }
}

/// Contract for independently schedulable, deterministic reflection tasks.
pub trait ReflectionTask: Send + Sync {
    /// Returns the globally stable task identifier.
    fn id(&self) -> TaskId;

    /// Returns the static human-readable name of the reflection task.
    fn name(&self) -> &'static str;

    /// Returns the task classification kind.
    fn kind(&self) -> ReflectionTaskKind;

    /// Returns the task-specific retry policy.
    fn retry_policy(&self) -> TaskRetryPolicy {
        TaskRetryPolicy::default()
    }

    /// Executes the task deterministically on canonical entities.
    fn execute(&self, entities: &mut Vec<CanonicalEntity>) -> TaskReport;

    /// Clones the task into a boxed trait object.
    fn clone_box(&self) -> Box<dyn ReflectionTask>;
}

impl Clone for Box<dyn ReflectionTask> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
