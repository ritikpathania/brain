# Execution Runtime Design Specification (RFC 1-6)

## Executive Summary

The **Execution Runtime** provides a crash-resilient, event-sourced orchestration engine for long-running workflows in `brain`. It unifies Milestone **R23 (Crash Recovery & Checkpointing)** and Milestone **R24 (Operational State & Lifecycle)** into a single core subsystem.

By decoupling the **Domain** (`Job`, business logic) from the **Runtime** (`Execution`, `Task`, scheduling, leases, checkpoints), the runtime ensures zero framework pollution in domain entities while establishing an append-only Write-Ahead Log (WAL) on SQLite.

---

## 1. RFC-1: Architecture & Entity Hierarchy

The runtime enforces a clean 4-tier separation:

```text
Execution (Workflow Aggregate Root)
    │
    ├── Task (Schedulable Runtime Node in Execution Graph) ──► Job Handler ──► Job (Domain Model)
    │      │
    │      └── Step (Internal Job Implementation Detail)
    └── Task ──► Job Handler ──► Job
```

### Core Definitions
1. **`Execution`**: The workflow aggregate root. Owns overall workflow lifecycle state, execution graph topology, version/revision tracking, global checkpoints, and recovery.
2. **`Task`**: A schedulable vertex within an Execution Graph. Manages readiness, retries, worker leases, and dependency satisfaction.
3. **`Job`**: Pure domain entity in `brain-domain`. Contains business inputs, outputs, and validation rules without any awareness of runtime scheduling or checkpoints.
4. **`Job Handler`**: The application handler invoked by a `Task` to execute a `Job`.
5. **`Step`**: Internal execution phase inside a `Job Handler`. Completely invisible to the scheduler, dependencies, and retry engine.

### Execution Identity
To support nested workflows, audit chains, and distributed tracing, executions carry a multi-faceted identity header:
* `ExecutionId`: Unique identifier for this execution instance.
* `ParentExecutionId`: Optional identifier of the parent execution if spawned by a parent workflow.
* `RootExecutionId`: Top-level execution identifier across nested hierarchies.
* `CorrelationId`: External context/session trace identifier.
* `CauseId`: Event or trigger identifier that caused this execution to spawn.

### Formal Ownership Responsibilities
| Component | Primary Responsibilities |
| :--- | :--- |
| **`Execution`** | Owns lifecycle state, graph topology, completion determination, versioning, and checkpoints. |
| **`Task`** | Owns scheduling state, worker leases, retry counts, and heartbeat tracking. |
| **`Scheduler`** | Owns queue ordering, ready set derivation, worker assignment, and lease acquisition. |
| **`Execution Aggregator`** | Owns deterministic event projection and workflow completion evaluation. |
| **`Retry Policy`** | Decoupled policy evaluator that intercepts `TaskFailed` and determines whether to emit `TaskRetryScheduled`. |

### Invariants
* **Execution Graph Ownership**: An `Execution` owns a Directed Acyclic Graph (DAG) of `Task` vertices.
* **Decoupled Task Dependencies**: Tasks never reference other tasks directly inside task structs. All graph edges are owned by a dedicated `task_dependency` relation.
* **Domain Isolation**: `brain-domain` models remain strictly free of async runtimes, database schemas, or WAL logic.

---

## 2. RFC-2: Execution & Task Lifecycle Finite State Machines

The runtime maintains two separate, complementary state machines.

### Execution FSM (Coarse Workflow Progress)
```text
Created
  │
  ▼
Queued
  │
  ▼
Running ◄───► Recovering
  │
  ├──► Checkpointing
  │
  ├──► Paused
  │
  ├──► Completed (Terminal)
  ├──► Failed (Terminal)
  └──► Cancelled (Terminal)
```

### Task FSM (Operational Worker Execution)
```text
Created
  │
  ▼
Waiting (Blocked by dependencies)
  │
  ▼
Ready (Eligible for scheduling)
  │
  ▼
Leased (Worker assigned)
  │
  ▼
Running
  │
  ├──► Checkpointing
  │
  ├──► Completed (Terminal)
  ├──► Skipped (Terminal - graph branch evaluation)
  ├──► Failed (Terminal - prior to retry policy evaluation)
  └──► Cancelled (Terminal)
```

> **Lifecycle Differentiation**: `Skipped` represents a workflow-driven DAG branch decision where a task is rendered unnecessary by a prior branch output. `Cancelled` implies external intervention.
>
> **Retry State Invariant**: `Retrying` is not a persistent Task state. When a task fails and the `Retry Policy` allows retrying: `TaskFailed` → `Retry Policy` emits `TaskRetryScheduled` → `Waiting` → `Ready`.

---

## 3. RFC-3: Event-Sourced Execution Journal (WAL) vs Ephemeral Event Bus

A critical architectural boundary exists between **Journal Events** (immutable WAL facts) and **Runtime Events** (ephemeral transport/UI notifications):

```text
Command / Worker Action
          │
          ▼
   Execution Runtime
          │
     ┌────┴─────────────────────────┐
     ▼                              ▼
Journal Event (Immutable)     Runtime Event (Ephemeral)
     │                              │
     ▼                              ▼
SQLite WAL Storage            Event Bus / UI Subscribers
```

* **Journal Events**: Persisted strictly to SQLite `execution_journal`. Form the sole source of truth for deterministic replay and recovery.
* **Runtime Events**: Ephemeral progress/telemetry signals (e.g. `TaskProgressUpdated`, `TuiStreamChunk`) broadcast to subscribers without polluting the WAL.

### Sequence Guarantees
* **Monotonic Ordering**: `sequence_no` is strictly increasing per `ExecutionId`.
* **Append-Only Immutability**: Journal events are strictly append-only; historical entries are never modified or reordered.
* **Optimistic Concurrency**: Executions maintain a monotonically increasing `version` (or `revision`) number. Any state mutation validates and increments the version.

### Journal Event Vocabulary

#### Execution Events
* `ExecutionCreated`: Workflow initialized with graph parameters and identity header.
* `ExecutionEnqueued`: Workflow queued for runtime scheduling.
* `ExecutionBegan`: First task execution started.
* `ExecutionCheckpointCreated`: Snapshot of execution graph state committed at sequence $N$.
* `ExecutionPaused`: Workflow execution explicitly paused.
* `ExecutionResumed`: Workflow execution resumed from paused state.
* `ExecutionRecovering`: Crash recovery sequence initiated.
* `ExecutionRecovered`: Replay complete and execution state restored.
* `ExecutionCompleted`: Workflow terminated successfully.
* `ExecutionFailed`: Workflow terminated with unrecoverable failure.
* `ExecutionCancelled`: Workflow cancelled by user/system request.

#### Task Events
* `TaskCreated`: Task vertex registered in execution graph.
* `TaskDependencySatisfied`: All prerequisite tasks completed.
* `TaskBecameReady`: Task moved to ready set for worker scheduling.
* `TaskLeased`: Worker acquired lease on task.
* `LeaseExpired`: Task lease expired without heartbeat.
* `TaskBegan`: Worker started executing job handler.
* `TaskHeartbeat`: Periodic worker liveness ping emitted.
* `TaskCheckpointCreated`: Task-level intermediate checkpoint saved.
* `TaskCompleted`: Job handler completed successfully.
* `TaskSkipped`: Task bypassed due to DAG branch execution logic.
* `TaskRetryScheduled`: Task failure intercepted by retry policy; reschedule queued.
* `TaskFailed`: Job handler failed (retries exhausted or fatal).
* `TaskCancelled`: Task execution cancelled.

### Deterministic Execution Aggregator
The **Execution Aggregator** is a pure projection:
```text
execution_journal ──► Execution Aggregator ──► Execution State Projection
```
Tasks never mutate `Execution` directly. The `Execution Aggregator` processes task completion events to update `Execution` FSM, evaluate graph completion, increment execution `version`, and request scheduler actions.

---

## 4. RFC-4: Durable Scheduler Architecture

### Queue-Free State Derivation
The scheduler does **not** persist a separate queue data structure. SQLite persists durable state facts (`task`, `task_dependency`); operational queues and ready sets are derived in-memory on startup.

### Worker Lease Protocol
Task leases are acquired atomically in SQLite:
* Worker attempts lease acquisition on a `READY` task or a task with `lease_until < NOW`.
* Upon success, task status updates to `LEASED` and `lease_until` is refreshed.
* Periodic `TaskHeartbeat` events extend the active lease.
* If a worker crashes, the lease expires naturally, and the scheduler reschedules the task on the next polling pass.
* Single-node operation defaults worker ID to `local-worker-1`, ensuring seamless multi-worker distribution (R25) without schema changes.

---

## 5. RFC-5: Storage Persistence & Recovery Engine

### Checkpoint Contract
A checkpoint snapshot represents the exact execution graph state **immediately following journal sequence $N$**.
```text
Checkpoint (Journal Sequence = N)
```
During recovery, the engine restores the checkpoint snapshot and replays only journal events with `sequence_no > N`.

### Startup Recovery Protocol
1. **Database Initialization**: Open SQLite database with WAL mode enabled.
2. **Active Execution Scan**: Query executions in `RUNNING` or `RECOVERING` states.
3. **Execution Recovery Phase**:
   - Transition execution to `ExecutionRecovering`.
   - Load latest `execution_checkpoint` snapshot (sequence $N$).
   - Replay `execution_journal` entries where `sequence_no > N`.
   - Reconstruct Execution Graph, Task FSM states, and execution `version`.
   - Emit `ExecutionRecovered` event.
4. **Queue Hydration**: Hydrate in-memory priority queue from tasks where `status = 'READY'`.
5. **Worker Resumption**: Re-assign ready tasks to worker loops.

---

## 6. RFC-6: Idempotency & Recovery Guarantees

| Guarantee | Description & Enforcement |
| :--- | :--- |
| **Deterministic Replay** | Replaying `execution_journal` from sequence $N$ yields identical FSM state and execution version. |
| **At-Least-Once Scheduling** | Task lease expiration guarantees crashed worker tasks are automatically rescheduled. |
| **Exactly-Once Command Effects** | Job handlers must be idempotent or leverage task execution tokens to avoid duplicate external side effects. |
| **Crash-Safe Checkpoints** | Checkpoints are committed atomically within SQLite transactions paired with exact `journal_sequence`. |
| **Zero State Drift** | In-memory ready queues are strictly derived from SQLite state; redundant persistent queue state is eliminated. |

---

## Appendix A: Recommended SQLite Schema Guidelines

*(Implementation details subject to migration updates during development)*

```sql
CREATE TABLE IF NOT EXISTS execution (
    execution_id TEXT PRIMARY KEY,
    parent_execution_id TEXT,
    root_execution_id TEXT NOT NULL,
    correlation_id TEXT,
    cause_id TEXT,
    status TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS execution_journal (
    sequence_no INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    occurred_at INTEGER NOT NULL,
    FOREIGN KEY(execution_id) REFERENCES execution(execution_id)
);

CREATE TABLE IF NOT EXISTS task (
    task_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL,
    status TEXT NOT NULL,
    priority INTEGER NOT NULL,
    lease_owner TEXT,
    lease_until INTEGER,
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(execution_id) REFERENCES execution(execution_id)
);

CREATE TABLE IF NOT EXISTS task_dependency (
    task_id TEXT NOT NULL,
    depends_on_task_id TEXT NOT NULL,
    PRIMARY KEY (task_id, depends_on_task_id),
    FOREIGN KEY(task_id) REFERENCES task(task_id),
    FOREIGN KEY(depends_on_task_id) REFERENCES task(task_id)
);

CREATE TABLE IF NOT EXISTS execution_checkpoint (
    checkpoint_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL,
    journal_sequence INTEGER NOT NULL,
    payload BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(execution_id) REFERENCES execution(execution_id)
);
```

---

## Verification & Test Plan

1. **Unit Tests (`brain-services`)**:
   - State machine transition tests (`ExecutionFsm`, `TaskFsm`) including `Skipped` and `Recovering`.
   - Event-sourced `Execution Aggregator` versioning and deterministic projection tests.
   - Retry Policy decoupling tests.
2. **Integration Tests (`crates/brain-services/tests/execution_runtime_tests.rs`)**:
   - Journal sequence replay determinism tests.
   - Crash simulation tests: simulate process kill mid-task, verify recovery engine restores state from checkpoint sequence $N$ and replays subsequent events without duplicate execution.
   - Lease expiration and re-scheduling integration tests.
