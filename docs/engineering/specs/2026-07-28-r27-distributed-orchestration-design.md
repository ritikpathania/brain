# Milestone R27 — Distributed Task Orchestration Architecture Specification

## Executive Summary

Milestone **R27 (Distributed Task Orchestration)** defines the coordinator-side orchestration engine for `brain`. Building directly on top of Milestone R25's [`WorkerTransport`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/src/distributed/transport.rs) and Milestone R26's [`TaskExecutor`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/superpowers/specs/2026-07-25-r26-worker-runtime-design.md#2-core-execution-traits--context) event pipeline, R27 introduces a unified dual-loop coordinator (`CoordinatorRuntime`), an immutable snapshot-based scheduling engine (`SchedulingEngine`), coordinator-side task queueing and admission control (`QueueManager`), lease lifecycle management (`LeaseManager`), progress tracking (`ProgressManager`), coordinator-side retries (`RetryCoordinator`), and worker failure detection (`FailureDetector`).

---

## 1. Architecture & Dual-Loop Pipeline

The **Coordinator Runtime** operates as a single-threaded state engine. It separates reactive business events from time-based maintenance:

```text
                           CoordinatorRuntime
                                   │
              ┌────────────────────┴────────────────────┐
              ▼                                         ▼
     Event Processing Loop                      Maintenance Loop
         (Reactive)                           (Periodic 1s Sweep)
              │                                         │
    External Messages                         Internal Maintenance
(TaskEnqueued, Heartbeat)                  (LeaseExpired, RetryDue)
              │                                         │
              └────────────────────┬────────────────────┘
                                   ▼
                         CoordinatorEvent Channel
                                   │
                      Unified Orchestration Pipeline
```

### Architectural Pipeline Invariants
> **1. Every `CoordinatorEvent` is processed atomically through a single FIFO pipeline. All coordinator state mutations occur before any outbound side effects are emitted.**
> **2. Outbound side effects (transport RPCs, database WAL appends, telemetry) must NEVER mutate `CoordinatorState` directly.**

```rust
use crate::distributed::models::*;
use crate::runtime::models::*;
use crate::worker::models::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalEvent {
    TaskEnqueued { task_id: TaskId, execution_id: ExecutionId, job_id: JobId, priority: u32 },
    WorkerRegistered { descriptor: WorkerDescriptor, status: WorkerStatus },
    HeartbeatReceived { heartbeat: WorkerHeartbeat },
    TaskExecutionEventReceived { event: TaskExecutionEvent },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternalEvent {
    LeaseExpired { task_id: TaskId, lease_id: u64 },
    WorkerLost { worker_id: String },
    WorkerRecovered { worker_id: String },
    RetryDue { task_id: TaskId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatorEvent {
    External(ExternalEvent),
    Internal(InternalEvent),
}
```

---

## 2. Root Aggregate & Component Ownership

The runtime owns a single root aggregate, `CoordinatorState`, containing 6 isolated subsystems:

```text
                               CoordinatorState
                                       │
     ┌───────────────────────┬─────────┴─────────────┬───────────────────────┐
     ▼                       ▼                       ▼                       ▼
QueueManager           WorkerRegistry          LeaseManager            ProgressManager
(QueueSnapshot)        (WorkerSnapshot)        (Active Leases)         (ExecutionProjection)
     │                       │                       │                       │
     └───────────────┬───────┘                       ▼                       ▼
                     ▼                        RetryCoordinator        FailureDetector
              SchedulingEngine
```

### Component Responsibilities
1. **`QueueManager`**: Owns task admission control, queue depth backpressure, priority ordering, and `QueueSnapshot` creation. Does not pick workers.
2. **`SchedulingEngine`**: Pure function computing `SchedulingDecision` placements over immutable `QueueSnapshot` and `WorkerSnapshot` inputs. Does not mutate storage or allocate leases.
3. **`LeaseManager`**: Owns task lease allocation, atomic `lease_id` updates, renewal, and lease expiration checks.
4. **`ProgressManager`**: Ingests worker progress events (`TaskExecutionEvent`) and updates pure [`ExecutionAggregator`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/src/runtime/aggregator.rs) projections.
5. **`RetryCoordinator`**: Handles coordinator-side task rescheduling for `ExecutionFailed`, `LeaseExpired`, and `WorkerLost` events.
6. **`FailureDetector`**: Monitors worker heartbeat timestamps independently of task lease state and emits `WorkerLost` / `WorkerRecovered` events.

---

## 3. Pure Scheduling Engine & Snapshot Matching

`SchedulingEngine` contains zero side effects, async IO, or database calls:

```rust
pub struct TaskNode {
    pub task_id: TaskId,
    pub execution_id: ExecutionId,
    pub job_id: JobId,
    pub priority: u32,
}

pub struct QueueSnapshot<'a> {
    pub ready_tasks: &'a [TaskNode],
}

pub struct WorkerSnapshot<'a> {
    pub candidates: &'a [WorkerCandidate<'a>],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulingDecision {
    Assign(TaskAssignment),
    Defer(TaskId),
    Reject(TaskId),
}

impl SchedulingEngine {
    pub fn schedule(
        &self,
        queue: &QueueSnapshot,
        workers: &WorkerSnapshot,
    ) -> Vec<SchedulingDecision> {
        // Pure placement evaluation matching task requirements to worker capabilities
    }
}
```
* **Decision Semantics**: `SchedulingDecision::Reject(TaskId)` represents a scheduler recommendation (e.g. policy violation, unschedulable requirements), not an automatic state machine failure.

---

## 4. Retry Coordinator & Failure Detector

### `RetryCoordinator` & Unified `RetryTrigger`
`RetryCoordinator` isolates coordinator-side rescheduling from worker-side execution retries (R26 `RetryExecutor`):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryTrigger {
    ExecutionFailed { task_id: TaskId, reason: String },
    LeaseExpired { task_id: TaskId, lease_id: u64 },
    WorkerLost { worker_id: String, active_tasks: Vec<TaskId> },
}
```
* **Rescheduling Flow**: `RetryTrigger` $\rightarrow$ Check attempt count $\rightarrow$ Produce `CoordinatorEvent::Internal(InternalEvent::RetryDue)`.

### Decoupled Worker Health vs Task Leases
- **Worker Health**: `FailureDetector` checks worker `last_seen_timestamp` against `heartbeat_timeout`. If exceeded, emits `WorkerLost(worker_id)`. Upon receiving a fresh heartbeat after timeout, emits `WorkerRecovered(worker_id)`.
- **Task Lease Expiry**: `LeaseManager` independently checks `lease_until` timestamps and emits `LeaseExpired(task_id, lease_id)`.

---

## 5. Pipeline Effect Guarantees & Future Evolution

### Outbound Effect Execution
Outbound effects (transport dispatches, WAL persistence, metrics, notifications) execute strictly after state mutations complete:

```text
1 CoordinatorEvent  ──►  0..N State Mutations  ──►  0..N Outbound Effects
  (FIFO Queue)           (CoordinatorState)          (Transport / Database)
```

### Future Evolution (`CoordinatorEffect`)
In future milestones (e.g. R28 High Availability), outbound effects will be explicitly reified as immutable structures:
```rust
pub enum CoordinatorEffect {
    Dispatch(TaskAssignment),
    Persist(JournalEvent),
    PublishTelemetry(TaskExecutionEvent),
}
```

---

## Verification & Test Plan

1. **Unit Tests (`crates/brain-services/src/coordinator/`)**:
   - `SchedulingEngine` pure function tests using mock `QueueSnapshot` and `WorkerSnapshot`.
   - `QueueManager` backpressure and priority sorting tests.
   - `LeaseManager` atomic lease creation and expiration sweep tests.
   - `FailureDetector` heartbeat timeout emission tests.
   - `RetryCoordinator` backoff and retry attempt policy tests.
2. **Integration Tests (`crates/brain-services/tests/r27_distributed_orchestration_tests.rs`)**:
   - End-to-end event pipeline test: `TaskEnqueued` $\rightarrow$ `SchedulingEngine` $\rightarrow$ `TaskAssignment` $\rightarrow$ `WorkerTransport` $\rightarrow$ `ProgressManager`.
   - Worker failure recovery test: simulate missed worker heartbeats, verify `WorkerLost` emission, lease cancellation, and `RetryCoordinator` task reassignment.
