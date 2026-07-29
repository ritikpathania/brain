//! `GlobalScheduler` Facade & Distributed Lease Fencing (Phase 10 Milestone 10.2).
//!
//! ### Architectural Invariants:
//! 1. Facade Delegation: `GlobalScheduler` acts strictly as a cluster-aware facade; it delegates placement algorithm execution to `ExecutionScheduler`.
//! 2. Monotonic Fence Generation: Fence tokens strictly increase across global task scheduling calls; tokens are never reused.
//! 3. Epoch Ownership: `FencedLease` inherits `EpochId` from `ClusterManager`.

use crate::planning::cluster::ClusterManager;
use crate::planning::fenced_lease::FencedLease;
use crate::planning::models::TaskId;
use crate::planning::scheduler::{ExecutionScheduler, RoundRobinPolicy, SchedulerError};
use crate::planning::supervision::CheckpointCapabilitySet;
use crate::planning::worker_registry::WorkerRegistry;

/// Global cluster scheduling facade delegating to `ExecutionScheduler` and granting `FencedLease` records.
pub struct GlobalScheduler {
    local_scheduler: ExecutionScheduler,
    next_fence_token: u64,
}

impl Default for GlobalScheduler {
    fn default() -> Self {
        Self::new(ExecutionScheduler::new(Box::new(RoundRobinPolicy::new())))
    }
}

impl GlobalScheduler {
    /// Instantiates a new `GlobalScheduler` wrapping an `ExecutionScheduler`.
    pub fn new(local_scheduler: ExecutionScheduler) -> Self {
        Self {
            local_scheduler,
            next_fence_token: 1,
        }
    }

    /// Schedules a task across the cluster, granting a monotonically fenced `FencedLease`.
    pub fn schedule_global_task(
        &mut self,
        task_id: TaskId,
        required_caps: &CheckpointCapabilitySet,
        registry: &WorkerRegistry,
        cluster: &ClusterManager,
        ttl_ms: u64,
        now_ms: u64,
    ) -> Result<FencedLease, SchedulerError> {
        // Delegate placement to local scheduler
        let lease =
            self.local_scheduler
                .schedule_task(task_id, required_caps, registry, ttl_ms, now_ms)?;

        let fence_token = self.next_fence_token;
        self.next_fence_token += 1;

        Ok(FencedLease::new(
            lease,
            cluster.current_epoch(),
            fence_token,
        ))
    }

    /// Returns a reference to the inner `ExecutionScheduler`.
    pub fn local_scheduler(&self) -> &ExecutionScheduler {
        &self.local_scheduler
    }
}
