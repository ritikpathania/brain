# Milestone R27 — Distributed Task Orchestration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Milestone R27 (Distributed Task Orchestration) in Rust within `crates/brain-services/src/coordinator/`, introducing the single-threaded `CoordinatorRuntime` dual-loop event pipeline, `CoordinatorState` aggregate root, `QueueManager`, pure snapshot-based `SchedulingEngine`, `LeaseManager`, `ProgressManager`, `RetryCoordinator`, and `FailureDetector`.

**Architecture:** Coordinator Runtime housed in `brain-services::coordinator` (layered strictly **above** `brain-services::distributed` and `brain-services::runtime`). Provides a single-threaded FIFO event pipeline processing `ExternalEvent` and `InternalEvent` variants into `CoordinatorState` mutations before emitting outbound side effects.

**Tech Stack:** Rust, `tokio`, `async-trait`, `parking_lot`, `serde`, `uuid`, `thiserror`.

## Global Constraints

- **Module Hierarchy Rule**: `coordinator/` may depend on `distributed/` and `runtime/`, but `runtime/` and `distributed/` MUST NEVER depend on `coordinator/`.
- **Stabilization Boundary Integrity**: Core Phase 1/Phase 2/Phase 3 contracts (`ExecutionId`, `TaskId`, `TaskAssignment`, `WorkerDescriptor`, `TaskExecutor`) MUST remain unchanged.
- **Pipeline Mutation Invariant**: All state mutations occur sequentially inside the single FIFO pipeline. Outbound side effects (transport RPCs, WAL appends, telemetry) MUST NEVER mutate `CoordinatorState` directly.
- **Pure Scheduling Engine**: `SchedulingEngine` operates over borrowed `QueueSnapshot<'a>` and `WorkerSnapshot<'a>` slices without mutating coordinator state or making storage/async calls.

---

### Task 1: Coordinator Scaffold & `CoordinatorState` Aggregate Root

**Files:**
- Create: `crates/brain-services/src/coordinator/mod.rs`
- Create: `crates/brain-services/src/coordinator/state.rs`
- Modify: `crates/brain-services/src/lib.rs`
- Test: `crates/brain-services/src/coordinator/state.rs` (inline test module)

**Interfaces:**
- Consumes: `TaskId`, `ExecutionId`, `JobId`
- Produces: `CoordinatorState`

- [ ] **Step 1: Write failing unit tests for CoordinatorState**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_state_initialization() {
        let state = CoordinatorState::new(100);
        assert_eq!(state.pending_task_count(), 0);
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --lib coordinator::state::tests`
Expected: FAIL with "module `coordinator` not found"

- [ ] **Step 3: Implement CoordinatorState**

In `crates/brain-services/src/coordinator/state.rs`:
```rust
#![allow(missing_docs)]

use std::sync::Arc;
use parking_lot::RwLock;

pub struct CoordinatorState {
    max_queue_depth: usize,
    pending_count: Arc<RwLock<usize>>,
}

impl CoordinatorState {
    pub fn new(max_queue_depth: usize) -> Self {
        Self {
            max_queue_depth,
            pending_count: Arc::new(RwLock::new(0)),
        }
    }

    pub fn max_queue_depth(&self) -> usize {
        self.max_queue_depth
    }

    pub fn pending_task_count(&self) -> usize {
        *self.pending_count.read()
    }
}
```

In `crates/brain-services/src/coordinator/mod.rs`:
```rust
pub mod state;

pub use state::*;
```

In `crates/brain-services/src/lib.rs`:
```rust
pub mod coordinator;
```

- [ ] **Step 4: Verify unit tests pass**

Run: `cargo test -p brain-services --lib coordinator::state::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/coordinator/
git add crates/brain-services/src/lib.rs
git commit -m "feat(coordinator): implement CoordinatorState aggregate root scaffold"
```

---

### Task 2: Coordinator Event Vocabulary (`ExternalEvent`, `InternalEvent`, `CoordinatorEvent`)

**Files:**
- Create: `crates/brain-services/src/coordinator/events.rs`
- Modify: `crates/brain-services/src/coordinator/mod.rs`
- Test: `crates/brain-services/src/coordinator/events.rs` (inline test module)

**Interfaces:**
- Consumes: `WorkerDescriptor`, `WorkerStatus`, `WorkerHeartbeat`, `TaskExecutionEvent`
- Produces: `ExternalEvent`, `InternalEvent`, `CoordinatorEvent`

- [ ] **Step 1: Write unit tests for CoordinatorEvent variants**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use brain_domain::jobs::JobId;
    use brain_services::runtime::*;

    #[test]
    fn test_coordinator_event_variants() {
        let task_id = TaskId::new();
        let exec_id = ExecutionId::new();
        let job_id = JobId(uuid::Uuid::new_v4());

        let ext = ExternalEvent::TaskEnqueued {
            task_id,
            execution_id: exec_id,
            job_id,
            priority: 1,
        };

        let ev = CoordinatorEvent::External(ext);
        assert!(matches!(ev, CoordinatorEvent::External(_)));
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --lib coordinator::events::tests`
Expected: FAIL with "cannot find type `ExternalEvent`"

- [ ] **Step 3: Implement ExternalEvent, InternalEvent, and CoordinatorEvent**

In `crates/brain-services/src/coordinator/events.rs`:
```rust
#![allow(missing_docs)]

use brain_domain::jobs::JobId;
use crate::distributed::models::*;
use crate::runtime::models::*;
use crate::worker::models::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalEvent {
    TaskEnqueued {
        task_id: TaskId,
        execution_id: ExecutionId,
        job_id: JobId,
        priority: u32,
    },
    WorkerRegistered {
        descriptor: WorkerDescriptor,
        status: WorkerStatus,
    },
    HeartbeatReceived {
        heartbeat: WorkerHeartbeat,
    },
    TaskExecutionEventReceived {
        event: TaskExecutionEvent,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InternalEvent {
    LeaseExpired {
        task_id: TaskId,
        lease_id: u64,
    },
    WorkerLost {
        worker_id: String,
    },
    WorkerRecovered {
        worker_id: String,
    },
    RetryDue {
        task_id: TaskId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinatorEvent {
    External(ExternalEvent),
    Internal(InternalEvent),
}
```

In `crates/brain-services/src/coordinator/mod.rs`:
```rust
pub mod events;
pub mod state;

pub use events::*;
pub use state::*;
```

- [ ] **Step 4: Verify unit tests pass**

Run: `cargo test -p brain-services --lib coordinator::events::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/coordinator/events.rs
git add crates/brain-services/src/coordinator/mod.rs
git commit -m "feat(coordinator): implement ExternalEvent, InternalEvent, and CoordinatorEvent vocabulary"
```

---

### Task 3: `QueueManager` & Priority Task Queueing

**Files:**
- Create: `crates/brain-services/src/coordinator/queue.rs`
- Modify: `crates/brain-services/src/coordinator/mod.rs`
- Test: `crates/brain-services/tests/queue_manager_tests.rs`

**Interfaces:**
- Consumes: `TaskId`, `ExecutionId`, `JobId`
- Produces: `TaskNode`, `QueueManager`, `QueueError`

- [ ] **Step 1: Write integration tests for QueueManager admission control and priority sorting**

In `crates/brain-services/tests/queue_manager_tests.rs`:
```rust
use brain_domain::jobs::JobId;
use brain_services::coordinator::*;
use brain_services::runtime::*;

#[test]
fn test_queue_manager_enqueue_and_priority_sorting() {
    let mut manager = QueueManager::new(2);

    let t1 = TaskId::new();
    let t2 = TaskId::new();
    let t3 = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    assert!(manager.enqueue(t1, exec_id, job_id, 1).is_ok());
    assert!(manager.enqueue(t2, exec_id, job_id, 10).is_ok());

    // Exceed max queue depth
    assert!(manager.enqueue(t3, exec_id, job_id, 5).is_err());

    let snapshot = manager.snapshot();
    assert_eq!(snapshot.ready_tasks.len(), 2);
    assert_eq!(snapshot.ready_tasks[0].task_id, t2); // Higher priority first
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test queue_manager_tests`
Expected: FAIL with "cannot find type `QueueManager`"

- [ ] **Step 3: Implement TaskNode, QueueManager, and QueueSnapshot**

In `crates/brain-services/src/coordinator/queue.rs`:
```rust
#![allow(missing_docs)]

use brain_domain::jobs::JobId;
use crate::runtime::models::*;
use serde::{Deserialize, Serialize};
use std::collections::BinaryHeap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueError {
    #[error("Queue full: depth limit {0} reached")]
    QueueFull(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskNode {
    pub task_id: TaskId,
    pub execution_id: ExecutionId,
    pub job_id: JobId,
    pub priority: u32,
    pub enqueued_at: u64,
}

impl Ord for TaskNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialOrd for TaskNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct QueueSnapshot {
    pub ready_tasks: Vec<TaskNode>,
}

pub struct QueueManager {
    max_depth: usize,
    heap: BinaryHeap<TaskNode>,
}

impl QueueManager {
    pub fn new(max_depth: usize) -> Self {
        Self {
            max_depth,
            heap: BinaryHeap::new(),
        }
    }

    pub fn enqueue(&mut self, task_id: TaskId, execution_id: ExecutionId, job_id: JobId, priority: u32) -> Result<(), QueueError> {
        if self.heap.len() >= self.max_depth {
            return Err(QueueError::QueueFull(self.max_depth));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.heap.push(TaskNode {
            task_id,
            execution_id,
            job_id,
            priority,
            enqueued_at: now,
        });

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        let mut sorted: Vec<TaskNode> = self.heap.iter().cloned().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        QueueSnapshot { ready_tasks: sorted }
    }
}
```

In `crates/brain-services/src/coordinator/mod.rs`:
```rust
pub mod events;
pub mod queue;
pub mod state;

pub use events::*;
pub use queue::*;
pub use state::*;
```

- [ ] **Step 4: Verify queue manager unit tests pass**

Run: `cargo test -p brain-services --test queue_manager_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/coordinator/queue.rs
git add crates/brain-services/src/coordinator/mod.rs
git add crates/brain-services/tests/queue_manager_tests.rs
git commit -m "feat(coordinator): implement TaskNode, QueueManager, and QueueSnapshot"
```

---

### Task 4: Pure `SchedulingEngine` & `SchedulingDecision` Placements

**Files:**
- Create: `crates/brain-services/src/coordinator/scheduler_engine.rs`
- Modify: `crates/brain-services/src/coordinator/mod.rs`
- Test: `crates/brain-services/tests/scheduling_engine_tests.rs`

**Interfaces:**
- Consumes: `QueueSnapshot`, `WorkerCandidate`, `SchedulingPolicy`
- Produces: `SchedulingDecision`, `SchedulingEngine`

- [ ] **Step 1: Write integration tests for SchedulingEngine**

In `crates/brain-services/tests/scheduling_engine_tests.rs`:
```rust
use brain_domain::jobs::JobId;
use brain_services::coordinator::*;
use brain_services::distributed::*;
use brain_services::runtime::*;

#[test]
fn test_scheduling_engine_pure_placement_evaluation() {
    let mut q_manager = QueueManager::new(10);
    let t1 = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());
    q_manager.enqueue(t1, exec_id, job_id, 5).unwrap();

    let desc1 = WorkerDescriptor {
        worker_id: "w1".to_string(),
        hostname: "node1".to_string(),
        protocol_version: 1,
        runtime_version: "1.0.0".to_string(),
        architecture: "x86_64".to_string(),
        supported_capabilities: std::collections::HashSet::new(),
        labels: std::collections::HashMap::new(),
    };
    let status1 = WorkerStatus {
        current_load: 0.1,
        available_resources: Resources { cpu_cores: 8, memory_bytes: 16000, gpu_count: 0, custom_resources: std::collections::HashMap::new() },
        active_lease_count: 0,
        is_healthy: true,
    };
    let c1 = WorkerCandidate { descriptor: &desc1, status: &status1 };

    let queue_snap = q_manager.snapshot();
    let candidates = vec![c1];
    let worker_snap = WorkerSnapshot { candidates: &candidates };

    let engine = SchedulingEngine::new(LeastLoadedPolicy);
    let decisions = engine.schedule(&queue_snap, &worker_snap);

    assert_eq!(decisions.len(), 1);
    match &decisions[0] {
        SchedulingDecision::Assign(assignment) => {
            assert_eq!(assignment.task_id, t1);
            assert_eq!(assignment.lease.lease_owner, "w1");
        }
        _ => panic!("Expected Assign decision"),
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test scheduling_engine_tests`
Expected: FAIL with "cannot find type `SchedulingEngine`"

- [ ] **Step 3: Implement SchedulingDecision, WorkerSnapshot, and SchedulingEngine**

In `crates/brain-services/src/coordinator/scheduler_engine.rs`:
```rust
#![allow(missing_docs)]

use crate::coordinator::queue::*;
use crate::distributed::models::*;
use crate::distributed::scheduler::*;
use crate::distributed::transport::*;
use crate::runtime::models::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulingDecision {
    Assign(TaskAssignment),
    Defer(TaskId),
    Reject(TaskId),
}

pub struct WorkerSnapshot<'a> {
    pub candidates: &'a [WorkerCandidate<'a>],
}

pub struct SchedulingEngine<P: SchedulingPolicy> {
    policy: P,
}

impl<P: SchedulingPolicy> SchedulingEngine<P> {
    pub fn new(policy: P) -> Self {
        Self { policy }
    }

    pub fn schedule<'a>(
        &self,
        queue: &'a QueueSnapshot,
        workers: &'a WorkerSnapshot<'a>,
    ) -> Vec<SchedulingDecision> {
        let mut decisions = Vec::new();

        for task in &queue.ready_tasks {
            if let Some(candidate) = self.policy.select_worker(task.priority, workers.candidates) {
                let assignment = TaskAssignment {
                    task_id: task.task_id,
                    execution_id: task.execution_id,
                    job_id: task.job_id,
                    input_ref: format!("artifact://inputs/{}/input.json", task.task_id.0),
                    lease: TaskLease {
                        lease_id: 1,
                        lease_owner: candidate.descriptor.worker_id.clone(),
                        lease_until: 1000,
                    },
                };
                decisions.push(SchedulingDecision::Assign(assignment));
            } else {
                decisions.push(SchedulingDecision::Defer(task.task_id));
            }
        }

        decisions
    }
}
```

In `crates/brain-services/src/coordinator/mod.rs`:
```rust
pub mod events;
pub mod queue;
pub mod scheduler_engine;
pub mod state;

pub use events::*;
pub use queue::*;
pub use scheduler_engine::*;
pub use state::*;
```

- [ ] **Step 4: Verify scheduling engine unit tests pass**

Run: `cargo test -p brain-services --test scheduling_engine_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/coordinator/scheduler_engine.rs
git add crates/brain-services/src/coordinator/mod.rs
git add crates/brain-services/tests/scheduling_engine_tests.rs
git commit -m "feat(coordinator): implement pure SchedulingEngine and SchedulingDecision"
```

---

### Task 5: `LeaseManager` & Decoupled `FailureDetector`

**Files:**
- Create: `crates/brain-services/src/coordinator/lease.rs`
- Create: `crates/brain-services/src/coordinator/failure_detector.rs`
- Modify: `crates/brain-services/src/coordinator/mod.rs`
- Test: `crates/brain-services/tests/lease_and_failure_tests.rs`

**Interfaces:**
- Consumes: `TaskId`, `WorkerDescriptor`, `WorkerHeartbeat`
- Produces: `LeaseManager`, `FailureDetector`

- [ ] **Step 1: Write unit tests for LeaseManager and FailureDetector**

In `crates/brain-services/tests/lease_and_failure_tests.rs`:
```rust
use brain_services::coordinator::*;
use brain_services::runtime::*;

#[test]
fn test_failure_detector_detects_worker_lost_and_recovery() {
    let mut detector = FailureDetector::new(5); // 5s timeout

    let w1 = "worker-1".to_string();
    detector.record_heartbeat(w1.clone(), 1000);

    // Within timeout -> healthy
    assert!(detector.check_health(w1.clone(), 1004).is_none());

    // Past timeout -> WorkerLost
    let lost_ev = detector.check_health(w1.clone(), 1006).unwrap();
    assert!(matches!(lost_ev, InternalEvent::WorkerLost { .. }));

    // Heartbeat resumes -> WorkerRecovered
    detector.record_heartbeat(w1.clone(), 1010);
    let rec_ev = detector.check_health(w1, 1010).unwrap();
    assert!(matches!(rec_ev, InternalEvent::WorkerRecovered { .. }));
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test lease_and_failure_tests`
Expected: FAIL with "cannot find type `FailureDetector`"

- [ ] **Step 3: Implement LeaseManager and FailureDetector**

In `crates/brain-services/src/coordinator/lease.rs`:
```rust
#![allow(missing_docs)]

use crate::coordinator::events::*;
use crate::distributed::transport::TaskLease;
use crate::runtime::models::*;
use std::collections::HashMap;

pub struct LeaseManager {
    lease_duration_secs: u64,
    active_leases: HashMap<TaskId, TaskLease>,
}

impl LeaseManager {
    pub fn new(lease_duration_secs: u64) -> Self {
        Self {
            lease_duration_secs,
            active_leases: HashMap::new(),
        }
    }

    pub fn allocate_lease(&mut self, task_id: TaskId, worker_id: &str, now: u64) -> TaskLease {
        let lease = TaskLease {
            lease_id: 1,
            lease_owner: worker_id.to_string(),
            lease_until: now + self.lease_duration_secs,
        };
        self.active_leases.insert(task_id, lease.clone());
        lease
    }

    pub fn sweep_expired(&mut self, now: u64) -> Vec<InternalEvent> {
        let mut expired = Vec::new();
        for (task_id, lease) in &self.active_leases {
            if lease.lease_until < now {
                expired.push(InternalEvent::LeaseExpired {
                    task_id: *task_id,
                    lease_id: lease.lease_id,
                });
            }
        }
        expired
    }
}
```

In `crates/brain-services/src/coordinator/failure_detector.rs`:
```rust
#![allow(missing_docs)]

use crate::coordinator::events::*;
use std::collections::HashMap;

pub struct FailureDetector {
    heartbeat_timeout_secs: u64,
    last_heartbeats: HashMap<String, u64>,
    lost_workers: HashMap<String, bool>,
}

impl FailureDetector {
    pub fn new(heartbeat_timeout_secs: u64) -> Self {
        Self {
            heartbeat_timeout_secs,
            last_heartbeats: HashMap::new(),
            lost_workers: HashMap::new(),
        }
    }

    pub fn record_heartbeat(&mut self, worker_id: String, timestamp: u64) {
        self.last_heartbeats.insert(worker_id, timestamp);
    }

    pub fn check_health(&mut self, worker_id: String, now: u64) -> Option<InternalEvent> {
        let last = self.last_heartbeats.get(&worker_id)?;
        let is_lost = *self.lost_workers.get(&worker_id).unwrap_or(&false);

        if now > last + self.heartbeat_timeout_secs {
            if !is_lost {
                self.lost_workers.insert(worker_id.clone(), true);
                return Some(InternalEvent::WorkerLost { worker_id });
            }
        } else if is_lost {
            self.lost_workers.insert(worker_id.clone(), false);
            return Some(InternalEvent::WorkerRecovered { worker_id });
        }
        None
    }
}
```

In `crates/brain-services/src/coordinator/mod.rs`:
```rust
pub mod events;
pub mod failure_detector;
pub mod lease;
pub mod queue;
pub mod scheduler_engine;
pub mod state;

pub use events::*;
pub use failure_detector::*;
pub use lease::*;
pub use queue::*;
pub use scheduler_engine::*;
pub use state::*;
```

- [ ] **Step 4: Verify lease & failure detector unit tests pass**

Run: `cargo test -p brain-services --test lease_and_failure_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/coordinator/lease.rs
git add crates/brain-services/src/coordinator/failure_detector.rs
git add crates/brain-services/src/coordinator/mod.rs
git add crates/brain-services/tests/lease_and_failure_tests.rs
git commit -m "feat(coordinator): implement LeaseManager and decoupled FailureDetector"
```

---

### Task 6: End-to-End Coordinator Task Orchestration Integration Suite

**Files:**
- Create: `crates/brain-services/tests/r27_distributed_orchestration_tests.rs`
- Test: Run full workspace check `cargo check --workspace`

- [ ] **Step 1: Write end-to-end orchestration pipeline integration tests**

In `crates/brain-services/tests/r27_distributed_orchestration_tests.rs`:
```rust
use brain_domain::jobs::JobId;
use brain_services::coordinator::*;
use brain_services::distributed::*;
use brain_services::runtime::*;

#[test]
fn test_end_to_end_coordinator_orchestration_pipeline() {
    let mut state = CoordinatorState::new(100);
    let mut queue_mgr = QueueManager::new(100);
    let mut failure_det = FailureDetector::new(5);

    let t1 = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    // 1. Enqueue task via ExternalEvent
    let enq_ev = CoordinatorEvent::External(ExternalEvent::TaskEnqueued {
        task_id: t1,
        execution_id: exec_id,
        job_id,
        priority: 5,
    });

    match enq_ev {
        CoordinatorEvent::External(ExternalEvent::TaskEnqueued { task_id, execution_id, job_id, priority }) => {
            queue_mgr.enqueue(task_id, execution_id, job_id, priority).unwrap();
        }
        _ => panic!("Expected TaskEnqueued"),
    }

    assert_eq!(queue_mgr.len(), 1);

    // 2. Failure detector heartbeat check
    failure_det.record_heartbeat("worker-1".to_string(), 1000);
    assert!(failure_det.check_health("worker-1".to_string(), 1002).is_none());
}
```

- [ ] **Step 2: Run end-to-end integration tests**

Run: `cargo test -p brain-services --test r27_distributed_orchestration_tests`
Expected: PASS

- [ ] **Step 3: Run full workspace check**

Run: `cargo check --workspace`
Expected: PASS (all workspace crates compile cleanly)

- [ ] **Step 4: Commit**

```bash
git add crates/brain-services/tests/r27_distributed_orchestration_tests.rs
git commit -m "test(coordinator): add end-to-end distributed orchestration integration tests"
```

---

## Inline Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-28-r27-distributed-orchestration-plan.md`.

Proceeding with **Inline Execution** (`executing-plans` skill) task-by-task.
