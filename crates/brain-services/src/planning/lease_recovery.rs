//! Assignment-Oriented `LeaseRecoveryEngine` and Pure `RecoveryPolicy` Decision Engine (Phase 9 Milestone 9.3).
//!
//! ### Architectural Invariants:
//! 1. Pure Decision Function: `RecoveryPolicy::determine_action` is strictly side-effect free; it inspects `RecoveryContext` and returns `RecoveryAction` without mutating runtime state.
//! 2. Encapsulated Context: `RecoveryContext` holds assignment, lease, heartbeat timestamp, and current time.
//! 3. Single Active Lease: Reassignment invalidates prior lease (`LeaseState::Expired` or `LeaseState::Released`) before issuing a new lease.

use crate::planning::scheduler::{ExecutionScheduler, SchedulerError, TaskAssignment, WorkerLease};
use crate::planning::supervision::CheckpointCapabilitySet;
use crate::planning::worker_registry::WorkerRegistry;
use serde::{Deserialize, Serialize};

/// Action decision produced by a `RecoveryPolicy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// Reassign task immediately to another capable active worker.
    ImmediateReassign,
    /// Wait for worker heartbeat before initiating recovery.
    WaitForHeartbeat,
    /// Attempt retry on original assigned worker node.
    RetryOriginalWorker,
    /// Escalate recovery failure to supervision control plane.
    Escalate,
}

/// Encapsulated context object provided to `RecoveryPolicy` decision evaluations.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryContext<'a> {
    /// Target task assignment.
    pub assignment: &'a TaskAssignment,
    /// Associated worker lease if present.
    pub lease: Option<&'a WorkerLease>,
    /// Last recorded heartbeat timestamp for assigned worker.
    pub last_heartbeat_ms: Option<u64>,
    /// Evaluation timestamp in milliseconds.
    pub now_ms: u64,
}

/// Trait evaluating recovery decisions side-effect free.
pub trait RecoveryPolicy: Send + Sync {
    /// Inspects `RecoveryContext` and returns recommended `RecoveryAction`.
    fn determine_action(&self, ctx: &RecoveryContext) -> RecoveryAction;
}

/// Policy executing immediate task step reassignment.
#[derive(Debug, Clone, Default)]
pub struct ImmediateReassignPolicy;

impl RecoveryPolicy for ImmediateReassignPolicy {
    fn determine_action(&self, _ctx: &RecoveryContext) -> RecoveryAction {
        RecoveryAction::ImmediateReassign
    }
}

/// Policy granting a grace period for heartbeat before reassigning.
#[derive(Debug, Clone)]
pub struct HeartbeatGracePolicy {
    /// Grace duration in milliseconds.
    pub grace_period_ms: u64,
}

impl Default for HeartbeatGracePolicy {
    fn default() -> Self {
        Self {
            grace_period_ms: 5000,
        }
    }
}

impl RecoveryPolicy for HeartbeatGracePolicy {
    fn determine_action(&self, ctx: &RecoveryContext) -> RecoveryAction {
        match ctx.last_heartbeat_ms {
            Some(last_hb) if ctx.now_ms.saturating_sub(last_hb) <= self.grace_period_ms => {
                RecoveryAction::WaitForHeartbeat
            }
            _ => RecoveryAction::ImmediateReassign,
        }
    }
}

/// Engine managing assignment-oriented lease recovery and reassignment.
#[derive(Debug, Clone, Default)]
pub struct LeaseRecoveryEngine;

impl LeaseRecoveryEngine {
    /// Instantiates a new `LeaseRecoveryEngine`.
    pub fn new() -> Self {
        Self
    }

    /// Recovers a failed or expired `TaskAssignment` by granting a new `WorkerLease` to a capable active worker.
    #[allow(clippy::too_many_arguments)]
    pub fn recover_assignment(
        &mut self,
        assignment: &TaskAssignment,
        old_lease: Option<&WorkerLease>,
        scheduler: &mut ExecutionScheduler,
        registry: &WorkerRegistry,
        required_caps: &CheckpointCapabilitySet,
        ttl_ms: u64,
        now_ms: u64,
    ) -> Result<WorkerLease, SchedulerError> {
        // 1. Invalidate old lease if provided
        if let Some(l) = old_lease {
            let _ = scheduler.release_lease(l.lease_id, now_ms);
        }

        // 2. Schedule new lease for assignment task
        scheduler.schedule_task(assignment.task_id, required_caps, registry, ttl_ms, now_ms)
    }
}
