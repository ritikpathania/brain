//! Distributed Placement Engine (`ExecutionScheduler`), Worker Leasing (`WorkerLease`), and `SchedulingPolicy` (Phase 9 Milestone 9.1).
//!
//! ### Architectural Invariants:
//! 1. Policy Separation: `SchedulingPolicy` trait owns placement algorithm (`select_worker`); `ExecutionScheduler` orchestrates assignment and leases.
//! 2. Assignment vs Lease: `TaskAssignment` represents placement intent; `WorkerLease` represents temporal ownership with explicit `LeaseState`.
//! 3. Immutable Worker Discovery: Scheduler reads candidate worker snapshots from `WorkerRegistry`; registry remains descriptive.
//! 4. Dedicated Event Vocabulary: Append-only `SchedulingEvent` stream tracks control placement decisions independently from execution/supervision events.
//! 5. Idempotent Operations: Lease release is idempotent; expired leases return `SchedulerError::LeaseExpired`.

use crate::planning::models::TaskId;
use crate::planning::supervision::CheckpointCapabilitySet;
use crate::planning::worker_registry::{ExecutionWorker, WorkerId, WorkerRegistry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

/// Strongly-typed lease identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LeaseId(pub Uuid);

impl std::fmt::Display for LeaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lease_{}", self.0)
    }
}

/// Strongly-typed scheduling event identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SchedulingEventId(pub Uuid);

impl std::fmt::Display for SchedulingEventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sched_ev_{}", self.0)
    }
}

/// Explicit operational state of a `WorkerLease`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LeaseState {
    /// Lease active and valid.
    Active,
    /// Lease explicitly released by worker/supervisor.
    Released,
    /// Lease TTL expired.
    Expired,
}

/// Placement assignment linking a task to a worker node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskAssignment {
    /// Unique assignment ID.
    pub assignment_id: Uuid,
    /// Target task ID.
    pub task_id: TaskId,
    /// Assigned worker ID.
    pub worker_id: WorkerId,
    /// Assignment timestamp in milliseconds.
    pub assigned_at_ms: u64,
}

/// Temporal ownership lease granted to a worker for task execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerLease {
    /// Unique lease ID.
    pub lease_id: LeaseId,
    /// Parent assignment ID.
    pub assignment_id: Uuid,
    /// Assigned worker ID.
    pub worker_id: WorkerId,
    /// Target task ID.
    pub task_id: TaskId,
    /// Current lease state.
    pub state: LeaseState,
    /// Issue timestamp in milliseconds.
    pub issued_at_ms: u64,
    /// Time-to-live bound in milliseconds.
    pub ttl_ms: u64,
}

impl WorkerLease {
    /// Evaluates if the lease has exceeded its TTL bound at specified timestamp.
    pub fn is_expired(&self, current_time_ms: u64) -> bool {
        if self.state == LeaseState::Expired {
            return true;
        }
        current_time_ms >= self.issued_at_ms + self.ttl_ms
    }
}

/// Strongly-typed error classification for scheduling and leasing operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulerError {
    /// No active worker satisfied required capabilities.
    NoCapableWorkerAvailable,
    /// Target worker is offline.
    WorkerOffline(WorkerId),
    /// Target lease not found.
    LeaseNotFound(LeaseId),
    /// Target lease is already expired.
    LeaseExpired(LeaseId),
    /// Target lease is already released.
    LeaseAlreadyReleased(LeaseId),
}

impl std::fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCapableWorkerAvailable => write!(f, "No capable active worker available"),
            Self::WorkerOffline(id) => write!(f, "Worker '{}' is offline", id),
            Self::LeaseNotFound(id) => write!(f, "Lease '{}' not found", id),
            Self::LeaseExpired(id) => write!(f, "Lease '{}' expired", id),
            Self::LeaseAlreadyReleased(id) => write!(f, "Lease '{}' already released", id),
        }
    }
}

impl std::error::Error for SchedulerError {}

/// Trait defining placement policy algorithms.
pub trait SchedulingPolicy: Send + Sync {
    /// Selects an optimal candidate worker from a list of capable candidates.
    fn select_worker(&self, candidates: &[&ExecutionWorker]) -> Result<WorkerId, SchedulerError>;
}

/// Round-robin placement policy.
#[derive(Debug, Default)]
pub struct RoundRobinPolicy {
    counter: AtomicUsize,
}

impl RoundRobinPolicy {
    /// Instantiates a new `RoundRobinPolicy`.
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

impl SchedulingPolicy for RoundRobinPolicy {
    fn select_worker(&self, candidates: &[&ExecutionWorker]) -> Result<WorkerId, SchedulerError> {
        if candidates.is_empty() {
            return Err(SchedulerError::NoCapableWorkerAvailable);
        }

        let idx = self.counter.fetch_add(1, Ordering::SeqCst) % candidates.len();
        Ok(candidates[idx].worker_id)
    }
}

/// Least-busy placement policy favoring active non-busy workers.
#[derive(Debug, Default)]
pub struct LeastBusyPolicy;

impl SchedulingPolicy for LeastBusyPolicy {
    fn select_worker(&self, candidates: &[&ExecutionWorker]) -> Result<WorkerId, SchedulerError> {
        if candidates.is_empty() {
            return Err(SchedulerError::NoCapableWorkerAvailable);
        }

        // Prefer Active status over Busy status
        let best = candidates
            .iter()
            .min_by_key(|w| match w.status {
                crate::planning::worker_registry::WorkerStatus::Active => 0,
                crate::planning::worker_registry::WorkerStatus::Busy => 1,
                crate::planning::worker_registry::WorkerStatus::Offline => 2,
            })
            .ok_or(SchedulerError::NoCapableWorkerAvailable)?;

        Ok(best.worker_id)
    }
}

/// Event kind classification for scheduling events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SchedulingEventKind {
    /// Task scheduled for placement.
    TaskScheduled,
    /// Worker node selected by policy.
    WorkerSelected,
    /// Worker lease granted.
    LeaseGranted,
    /// Worker lease renewed.
    LeaseRenewed,
    /// Worker lease released.
    LeaseReleased,
    /// Worker lease expired.
    LeaseExpired,
}

/// Single append-only event item in the scheduling log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulingEvent {
    /// Unique scheduling event ID.
    pub event_id: SchedulingEventId,
    /// Event classification kind.
    pub kind: SchedulingEventKind,
    /// Descriptive message text.
    pub message: String,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Distributed scheduler orchestrating task placement, assignments, worker leases, and scheduling logs.
pub struct ExecutionScheduler {
    policy: Box<dyn SchedulingPolicy>,
    assignments: HashMap<Uuid, TaskAssignment>,
    leases: HashMap<LeaseId, WorkerLease>,
    events: Vec<SchedulingEvent>,
}

impl Default for ExecutionScheduler {
    fn default() -> Self {
        Self::new(Box::new(RoundRobinPolicy::new()))
    }
}

impl ExecutionScheduler {
    /// Instantiates a new `ExecutionScheduler` with specified `SchedulingPolicy`.
    pub fn new(policy: Box<dyn SchedulingPolicy>) -> Self {
        Self {
            policy,
            assignments: HashMap::new(),
            leases: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Returns the append-only scheduling event log.
    pub fn events(&self) -> &[SchedulingEvent] {
        &self.events
    }

    fn emit_event(&mut self, kind: SchedulingEventKind, msg: &str, timestamp_ms: u64) {
        self.events.push(SchedulingEvent {
            event_id: SchedulingEventId(Uuid::new_v4()),
            kind,
            message: msg.to_string(),
            timestamp_ms,
        });
    }

    /// Schedules a task, selecting a worker via policy and granting a `WorkerLease`.
    pub fn schedule_task(
        &mut self,
        task_id: TaskId,
        required_caps: &CheckpointCapabilitySet,
        registry: &WorkerRegistry,
        ttl_ms: u64,
        now_ms: u64,
    ) -> Result<WorkerLease, SchedulerError> {
        let candidates = registry.find_capable_workers(required_caps);
        if candidates.is_empty() {
            return Err(SchedulerError::NoCapableWorkerAvailable);
        }

        self.emit_event(
            SchedulingEventKind::TaskScheduled,
            &format!("Task '{}' requested placement", task_id),
            now_ms,
        );

        let selected_worker_id = self.policy.select_worker(&candidates)?;
        self.emit_event(
            SchedulingEventKind::WorkerSelected,
            &format!(
                "Worker '{}' selected for task '{}'",
                selected_worker_id, task_id
            ),
            now_ms + 1,
        );

        let assignment_id = Uuid::new_v4();
        let assignment = TaskAssignment {
            assignment_id,
            task_id,
            worker_id: selected_worker_id,
            assigned_at_ms: now_ms + 2,
        };

        self.assignments.insert(assignment_id, assignment);

        let lease_id = LeaseId(Uuid::new_v4());
        let lease = WorkerLease {
            lease_id,
            assignment_id,
            worker_id: selected_worker_id,
            task_id,
            state: LeaseState::Active,
            issued_at_ms: now_ms + 3,
            ttl_ms,
        };

        self.leases.insert(lease_id, lease.clone());
        self.emit_event(
            SchedulingEventKind::LeaseGranted,
            &format!(
                "Granted lease '{}' to worker '{}'",
                lease_id, selected_worker_id
            ),
            now_ms + 3,
        );

        Ok(lease)
    }

    /// Renews an active `WorkerLease`, extending its TTL bound.
    pub fn renew_lease(
        &mut self,
        lease_id: LeaseId,
        extension_ms: u64,
        now_ms: u64,
    ) -> Result<(), SchedulerError> {
        let lease = self
            .leases
            .get_mut(&lease_id)
            .ok_or(SchedulerError::LeaseNotFound(lease_id))?;

        if lease.state == LeaseState::Released {
            return Err(SchedulerError::LeaseAlreadyReleased(lease_id));
        }

        if lease.is_expired(now_ms) {
            lease.state = LeaseState::Expired;
            self.emit_event(
                SchedulingEventKind::LeaseExpired,
                &format!("Lease '{}' expired", lease_id),
                now_ms,
            );
            return Err(SchedulerError::LeaseExpired(lease_id));
        }

        lease.ttl_ms += extension_ms;
        self.emit_event(
            SchedulingEventKind::LeaseRenewed,
            &format!("Renewed lease '{}' by {}ms", lease_id, extension_ms),
            now_ms,
        );

        Ok(())
    }

    /// Releases a `WorkerLease` idempotently.
    pub fn release_lease(&mut self, lease_id: LeaseId, now_ms: u64) -> Result<(), SchedulerError> {
        let lease = self
            .leases
            .get_mut(&lease_id)
            .ok_or(SchedulerError::LeaseNotFound(lease_id))?;

        if lease.state == LeaseState::Released {
            return Ok(()); // Idempotent release
        }

        lease.state = LeaseState::Released;
        self.emit_event(
            SchedulingEventKind::LeaseReleased,
            &format!("Released lease '{}'", lease_id),
            now_ms,
        );

        Ok(())
    }
}
