//! Transport-Neutral `ExecutionDispatcher` Trait and `DeliveryAck` Acknowledgement (Phase 9 Milestone 9.2).
//!
//! ### Architectural Invariants:
//! 1. Transport Neutrality: `ExecutionDispatcher` separates placement (where) from task delivery (how).
//! 2. Delivery Acknowledgement: `dispatch_task` returns an explicit `DeliveryAck` separating dispatch from task execution.
//! 3. Lifecycle Invariant: Scheduled -> Dispatched -> Acknowledged -> Executing.

use crate::planning::execution_runtime::{DefaultTaskExecutor, ExecutionFailure, TaskExecutor};
use crate::planning::models::TaskStep;
use crate::planning::scheduler::{LeaseId, WorkerLease};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Receipt acknowledgement artifact emitted upon successful task step delivery to a worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeliveryAck {
    /// Unique acknowledgement ID.
    pub ack_id: Uuid,
    /// Associated worker lease ID.
    pub lease_id: LeaseId,
    /// Receipt timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Trait defining transport-neutral task step delivery contract.
pub trait ExecutionDispatcher: Send + Sync {
    /// Dispatches a `TaskStep` over an active `WorkerLease` and returns `Result<DeliveryAck, ExecutionFailure>`.
    fn dispatch_task(
        &self,
        lease: &WorkerLease,
        task: &TaskStep,
    ) -> Result<DeliveryAck, ExecutionFailure>;
}

/// Strongly-typed dispatch lifecycle event identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DispatchLifecycleEventId(pub Uuid);

impl std::fmt::Display for DispatchLifecycleEventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dispatch_ev_{}", self.0)
    }
}

/// Event kind classification for task delivery and step execution progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DispatchLifecycleEventKind {
    /// Task step execution started.
    TaskStepStarted,
    /// Task step execution reported progress percentage.
    TaskStepProgress,
    /// Task step execution completed successfully.
    TaskStepCompleted,
    /// Task step execution failed.
    TaskStepFailed,
}

/// Single append-only event item tracking task step delivery progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DispatchLifecycleEvent {
    /// Unique event ID.
    pub event_id: DispatchLifecycleEventId,
    /// Target worker lease ID.
    pub lease_id: LeaseId,
    /// Target task ID.
    pub task_id: crate::planning::models::TaskId,
    /// Event classification kind.
    pub kind: DispatchLifecycleEventKind,
    /// Optional task progress percentage (0.0 to 100.0).
    pub progress_percent: Option<f32>,
    /// Event timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Reference local in-memory implementation of `ExecutionDispatcher`.
pub struct LocalExecutionDispatcher {
    executor: Box<dyn TaskExecutor>,
}

impl Default for LocalExecutionDispatcher {
    fn default() -> Self {
        Self::new(Box::new(DefaultTaskExecutor))
    }
}

impl LocalExecutionDispatcher {
    /// Instantiates a new `LocalExecutionDispatcher` with specified `TaskExecutor`.
    pub fn new(executor: Box<dyn TaskExecutor>) -> Self {
        Self { executor }
    }
}

impl ExecutionDispatcher for LocalExecutionDispatcher {
    fn dispatch_task(
        &self,
        lease: &WorkerLease,
        task: &TaskStep,
    ) -> Result<DeliveryAck, ExecutionFailure> {
        if lease.state != crate::planning::scheduler::LeaseState::Active {
            return Err(ExecutionFailure {
                kind: crate::planning::execution_runtime::ExecutionFailureKind::TaskFailure,
                task_id: Some(task.task_id),
                message: format!("Cannot dispatch over non-active lease '{}'", lease.lease_id),
            });
        }

        self.executor.execute_task(task)?;

        Ok(DeliveryAck {
            ack_id: Uuid::new_v4(),
            lease_id: lease.lease_id,
            timestamp_ms: lease.issued_at_ms + 10,
        })
    }
}

/// Reference transport-neutral remote implementation of `ExecutionDispatcher`.
#[derive(Debug, Clone, Default)]
pub struct RemoteExecutionDispatcher;

impl ExecutionDispatcher for RemoteExecutionDispatcher {
    fn dispatch_task(
        &self,
        lease: &WorkerLease,
        task: &TaskStep,
    ) -> Result<DeliveryAck, ExecutionFailure> {
        if lease.state != crate::planning::scheduler::LeaseState::Active {
            return Err(ExecutionFailure {
                kind: crate::planning::execution_runtime::ExecutionFailureKind::TaskFailure,
                task_id: Some(task.task_id),
                message: format!("Cannot dispatch over non-active lease '{}'", lease.lease_id),
            });
        }

        Ok(DeliveryAck {
            ack_id: Uuid::new_v4(),
            lease_id: lease.lease_id,
            timestamp_ms: lease.issued_at_ms + 15,
        })
    }
}
