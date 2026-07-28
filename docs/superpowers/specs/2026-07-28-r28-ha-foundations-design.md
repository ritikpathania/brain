# Milestone R28 — High Availability Foundations Architecture Specification

## Executive Summary

Milestone **R28 (High Availability Foundations)** establishes the deterministic replay, intent logging, and effect materialization foundation for `brain`'s coordinator. Building directly on Milestone R27's [`CoordinatorRuntime`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/superpowers/specs/2026-07-28-r27-distributed-orchestration-design.md#1-architecture--dual-loop-pipeline) and [`CoordinatorState`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/src/coordinator/state.rs) root aggregate, R28 shifts the coordinator from *executing* side-effects to *deciding* side-effects.

R28 introduces semantic `CoordinatorDecision` structures, operational `CoordinatorEffect` items, `CoordinatorDecisionMaterializer`, a durable `IntentLog` WAL interface, an out-of-band `CoordinatorEffectExecutor`, and a crash-safe `IntentReplayEngine`.

---

## 1. Architecture & Intent Log Layering

The **Coordinator Runtime** decouples decision-making from side-effect execution. The coordinator pipeline ends at decision materialization; durability and side-effect execution are handled by dedicated infrastructure:

```text
                        CoordinatorEvent (FIFO)
                                   │
                         CoordinatorState Root
                                   │
                    CoordinatorDecision (Semantic)
                                   │
                    CoordinatorDecisionMaterializer
                                   │
                    CoordinatorEffect (Operational)
                                   │
                     ┌─────────────┴─────────────┐
                     ▼                           ▼
            IntentLog WAL (fsync)       CoordinatorEffectExecutor
                     │                           │
           Persisted Intent Record     ┌─────────┼─────────┐
                     │                 ▼         ▼         ▼
             Replay / Recovery     Worker RPC SQLite WAL Telemetry
```

### Central Architectural Invariants
> **1. Every externally observable effect MUST originate from a persisted `CoordinatorEffect`.**  
> **2. Every externally visible subsystem consuming a `CoordinatorEffect` MUST treat `EffectId` as the idempotency key.**  
> **3. `CoordinatorEffectExecutor` must NEVER mutate `CoordinatorState` directly.**  
> **4. Effects materialized from a single `CoordinatorDecision` MUST preserve their generation order when appended to the intent log.**

---

## 2. Intent Record Schema & Newtypes

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNumber(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EffectId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentStatus {
    Created,
    Persisted,
    Executing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinatorDecision {
    AssignTask { task_id: TaskId, worker_id: String },
    ExpireLease { task_id: TaskId, lease_id: u64 },
    RescheduleTask { task_id: TaskId, attempt: u32 },
    MarkWorkerLost { worker_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinatorEffect {
    Dispatch(TaskAssignment),
    Persist(JournalEvent),
    PublishTelemetry(TaskExecutionEvent),
    EmitWorkerLost(String),
    ScheduleRetry(TaskId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentRecord {
    pub sequence: SequenceNumber,
    pub event_id: EventId,
    pub effect_id: EffectId,
    pub created_at: u64,
    pub effect: CoordinatorEffect,
    pub status: IntentStatus,
}
```

### Intent Status State Machine
```text
Created
   │
Persisted (fsync)
   │
Executing
   ├──► Completed (Skip on Replay)
   └──► Failed (Retry Policy Evaluation)
```

### Materialization Policy
`CoordinatorDecisionMaterializer` expands one semantic `CoordinatorDecision` into 1..$N$ operational `CoordinatorEffect` items while preserving generation order:
```rust
pub struct CoordinatorDecisionMaterializer;

impl CoordinatorDecisionMaterializer {
    pub fn materialize(decision: CoordinatorDecision) -> Vec<CoordinatorEffect> {
        // Maps 1 semantic decision to 1..N operational effects in generation order
    }
}
```

---

## 3. Intent Log Storage Trait & Persistence

The `IntentLog` trait isolates coordinator durability from underlying storage engines:

```rust
#[derive(Debug, Error)]
pub enum IntentLogError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Duplicate sequence number {0:?}")]
    DuplicateSequence(SequenceNumber),
}

#[async_trait]
pub trait IntentLog: Send + Sync {
    async fn append_record(&self, record: &IntentRecord) -> Result<(), IntentLogError>;
    async fn update_status(&self, effect_id: EffectId, status: IntentStatus) -> Result<(), IntentLogError>;
    async fn load_from(&self, sequence: SequenceNumber) -> Result<Vec<IntentRecord>, IntentLogError>;
    async fn scan_pending(&self) -> Result<Vec<IntentRecord>, IntentLogError>;
}
```
* **Sequence Ownership**: The `IntentLog` implementation is strictly responsible for guaranteeing globally monotonic sequence allocation (`SequenceNumber`) within a coordinator instance.

---

## 4. Effect Executor & Side-Effect Routing

`CoordinatorEffectExecutor` owns operational side-effect routing:

```rust
#[derive(Debug, Error)]
pub enum EffectExecutionError {
    #[error("Transport failure: {0}")]
    Transport(String),
    #[error("Storage failure: {0}")]
    Storage(String),
}

#[async_trait]
pub trait CoordinatorEffectExecutor: Send + Sync {
    async fn execute_effect(&self, effect_id: EffectId, effect: &CoordinatorEffect) -> Result<(), EffectExecutionError>;
}
```

### Deterministic Effect Routing Table
| `CoordinatorEffect` | Target Subsystem | Idempotency Key |
| :--- | :--- | :--- |
| `Dispatch(TaskAssignment)` | [`WorkerTransport`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/src/distributed/transport.rs) | `EffectId` |
| `Persist(JournalEvent)` | [`ExecutionRepository`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/src/runtime/repository.rs) | `EffectId` |
| `PublishTelemetry(TaskExecutionEvent)` | Telemetry / Event Stream | `EffectId` |
| `EmitWorkerLost(String)` | Worker Telemetry Channel | `EffectId` |
| `ScheduleRetry(TaskId)` | Queue Manager / Retry Bus | `EffectId` |

---

## 5. Replay Engine & Crash Recovery Semantics

On coordinator startup or failover recovery, `IntentReplayEngine` scans pending records (implementations may stream or batch records internally while preserving global sequence order):

```text
Startup ──► IntentLog::scan_pending() ──► Replay strictly by Monotonic SequenceNumber
                                                       │
                 ┌─────────────────────────────────────┼─────────────────────────────────────┐
                 ▼                                     ▼                                     ▼
      IntentStatus::Completed                IntentStatus::Executing                IntentStatus::Failed
                 │                                     │                                     │
                 ▼                                     ▼                                     ▼
           Skip Record                      Re-execute Safely                     Evaluate Retry
       (Durable Finality)               (Idempotent via EffectId)                (Retry Policy)
```

### Status Ambiguity Definitions
- **`Executing`**: Represents effects whose completion status was interrupted by a coordinator crash. Replay MUST treat them as incomplete and rely on `EffectId` idempotency to safely re-execute.
- **`Completed`**: Represents durable finality. Replay skips `Completed` records without re-execution or verification.
- **`Failed`**: Replay passes `Failed` records to `RetryPolicy` for evaluation.

---

## Verification & Test Plan

1. **Unit Tests (`crates/brain-services/src/ha/`)**:
   - `CoordinatorDecisionMaterializer` decision-to-effect expansion tests.
   - `IntentRecord` serialization and status state machine transition tests.
   - `IntentReplayEngine` sequence-ordered replay and status filter tests.
2. **Integration Tests (`crates/brain-services/tests/r28_ha_foundations_tests.rs`)**:
   - Simulated crash during effect execution: verify `Executing` record re-execution using `EffectId` idempotency.
   - `Completed` status skip verification: ensure completed records are never re-executed upon restart.
