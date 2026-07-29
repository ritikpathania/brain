//! Domain models and error types for Execution Planning & Stage Partitioning (Phase 7 Milestone 7.3).
//!
//! ### Architectural Invariants:
//! 1. `ExecutionPlan` is the compiled scheduling artifact consumed by future execution engines.
//! 2. `ExecutionPlan` is compiled and **immutable**.
//! 3. Every task in `TaskPlan` appears in exactly one stage in `ExecutionPlan`.
//! 4. Stage indices are contiguous starting at zero (`0, 1, 2, ...`).
//! 5. Deterministic stage ordering: equal `TaskPlan` inputs produce identical `ExecutionPlan` artifacts.

use crate::planning::models::{PlanId, TaskId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Strongly-typed execution plan identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionPlanId(pub Uuid);

impl std::fmt::Display for ExecutionPlanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exec_plan_{}", self.0)
    }
}

/// Synchronization barrier classification for execution stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum BarrierKind {
    /// Strict barrier (all tasks in stage N must finish before stage N+1 starts).
    #[default]
    Strict,
    /// Soft barrier (pipelined execution allowed when dependencies are met).
    Soft,
}

/// A collection of independent task steps that can execute concurrently in parallel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionStage {
    /// 0-based contiguous stage index.
    pub stage_index: usize,
    /// Parallel task IDs assigned to this stage.
    pub parallel_tasks: Vec<TaskId>,
    /// Estimated resource or latency cost for this stage.
    pub estimated_cost: f32,
    /// Barrier synchronization kind.
    pub barrier_kind: BarrierKind,
}

/// Scheduling strategy preference for the execution planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SchedulingStrategy {
    /// Maximizes parallel concurrency by grouping all independent tasks into the earliest possible stage.
    #[default]
    MaximumParallelism,
    /// Minimizes peak resource usage by throttling parallel stage width.
    MinimumResource,
}

/// Configurable policy parameters for execution planning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExecutionPlanningPolicy {
    /// Scheduling strategy preference.
    pub strategy: SchedulingStrategy,
    /// Default stage barrier synchronization kind.
    pub default_barrier: BarrierKind,
}

impl ExecutionPlanningPolicy {
    /// Instantiates a default `ExecutionPlanningPolicy`.
    pub fn new() -> Self {
        Self {
            strategy: SchedulingStrategy::MaximumParallelism,
            default_barrier: BarrierKind::Strict,
        }
    }
}

/// Strongly-typed error classification for execution planning failures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionPlanningError {
    /// TaskGraph structure is invalid or empty.
    InvalidTaskGraph(String),
    /// Dependency cycle detected in TaskGraph.
    DependencyCycle(String),
    /// Expected task step missing from stage graph.
    MissingTask(TaskId),
    /// Invalid synchronization barrier specified.
    InvalidBarrier(String),
}

impl std::fmt::Display for ExecutionPlanningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTaskGraph(msg) => write!(f, "Invalid TaskGraph: {}", msg),
            Self::DependencyCycle(msg) => write!(f, "Dependency cycle in TaskGraph: {}", msg),
            Self::MissingTask(id) => write!(f, "Task '{}' missing from execution plan", id),
            Self::InvalidBarrier(msg) => write!(f, "Invalid execution barrier: {}", msg),
        }
    }
}

impl std::error::Error for ExecutionPlanningError {}

/// Compiled immutable `ExecutionPlan` artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Unique execution plan ID.
    pub execution_plan_id: ExecutionPlanId,
    /// Source compiled task plan ID.
    pub task_plan_id: PlanId,
    /// Contiguous, stage-ordered execution stages.
    pub stages: Vec<ExecutionStage>,
    /// Compilation timestamp in milliseconds.
    pub timestamp_ms: u64,
}

impl ExecutionPlan {
    /// Returns the total number of task steps across all stages.
    pub fn total_tasks(&self) -> usize {
        self.stages.iter().map(|s| s.parallel_tasks.len()).sum()
    }

    /// Returns the number of execution stages.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }
}
