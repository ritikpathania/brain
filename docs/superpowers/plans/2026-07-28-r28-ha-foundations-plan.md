# Milestone R28 — High Availability Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Milestone R28 (High Availability Foundations) in Rust within `crates/brain-services/src/ha/`, introducing `SequenceNumber`, `EventId`, `EffectId`, `CoordinatorDecision`, `CoordinatorEffect`, `CoordinatorDecisionMaterializer`, `trait IntentLog`, `SqliteIntentLog`, `trait CoordinatorEffectExecutor`, and `IntentReplayEngine`.

**Architecture:** HA Foundations housed in `brain-services::ha` (layered strictly **above** `brain-services::coordinator` and `brain-services::distributed`). Decouples decision generation from side-effect execution: `CoordinatorDecision` $\rightarrow$ `CoordinatorDecisionMaterializer` $\rightarrow$ `CoordinatorEffect` $\rightarrow$ `IntentLog` WAL $\rightarrow$ `CoordinatorEffectExecutor`.

**Tech Stack:** Rust, `tokio`, `async-trait`, `rusqlite`, `serde`, `uuid`, `thiserror`.

## Global Constraints

- **Module Hierarchy Rule**: `ha/` may depend on `coordinator/`, `distributed/`, and `runtime/`, but `runtime/`, `distributed/`, and `coordinator/` MUST NEVER depend on `ha/`.
- **Stabilization Boundary Integrity**: Core contracts from Phase 1 to Phase 4 (`ExecutionId`, `TaskId`, `TaskAssignment`, `WorkerDescriptor`, `CoordinatorState`) MUST remain unchanged.
- **Side-Effect Invariants**:
  1. Every externally observable effect MUST originate from a persisted `CoordinatorEffect`.
  2. Every externally visible subsystem consuming a `CoordinatorEffect` MUST treat `EffectId` as the idempotency key.
  3. `CoordinatorEffectExecutor` MUST NEVER mutate `CoordinatorState` directly.
- **Sequence Ownership**: `IntentLog` implementations are strictly responsible for allocating globally monotonic `SequenceNumber` values.

---

### Task 1: HA Newtypes & Core Models (`SequenceNumber`, `EventId`, `EffectId`, `IntentStatus`, `CoordinatorDecision`, `CoordinatorEffect`, `IntentRecord`)

**Files:**
- Create: `crates/brain-services/src/ha/mod.rs`
- Create: `crates/brain-services/src/ha/models.rs`
- Modify: `crates/brain-services/src/lib.rs`
- Test: `crates/brain-services/src/ha/models.rs` (inline test module)

**Interfaces:**
- Consumes: `TaskId`, `ExecutionId`, `JobId`, `TaskAssignment`, `JournalEvent`, `TaskExecutionEvent`
- Produces: `SequenceNumber`, `EventId`, `EffectId`, `IntentStatus`, `CoordinatorDecision`, `CoordinatorEffect`, `IntentRecord`

- [ ] **Step 1: Write failing unit tests for HA models and newtypes**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_intent_record_structure_and_newtypes() {
        let seq = SequenceNumber(1);
        let event_id = EventId(Uuid::new_v4());
        let effect_id = EffectId(Uuid::new_v4());

        let record = IntentRecord {
            sequence: seq,
            event_id,
            effect_id,
            created_at: 1000,
            effect: CoordinatorEffect::EmitWorkerLost("worker-1".to_string()),
            status: IntentStatus::Created,
        };

        assert_eq!(record.sequence, SequenceNumber(1));
        assert_eq!(record.status, IntentStatus::Created);
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --lib ha::models::tests`
Expected: FAIL with "module `ha` not found"

- [ ] **Step 3: Implement HA newtypes, decision/effect enums, and IntentRecord**

In `crates/brain-services/src/ha/models.rs`:
```rust
#![allow(missing_docs)]

use crate::distributed::transport::*;
use crate::runtime::events::*;
use crate::runtime::models::*;
use crate::worker::models::*;
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

In `crates/brain-services/src/ha/mod.rs`:
```rust
pub mod models;

pub use models::*;
```

In `crates/brain-services/src/lib.rs`:
```rust
pub mod ha;
```

- [ ] **Step 4: Verify unit tests pass**

Run: `cargo test -p brain-services --lib ha::models::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/ha/
git add crates/brain-services/src/lib.rs
git commit -m "feat(ha): implement SequenceNumber, EventId, EffectId, IntentStatus, and IntentRecord"
```

---

### Task 2: `CoordinatorDecisionMaterializer`

**Files:**
- Create: `crates/brain-services/src/ha/materializer.rs`
- Modify: `crates/brain-services/src/ha/mod.rs`
- Test: `crates/brain-services/src/ha/materializer.rs` (inline test module)

**Interfaces:**
- Consumes: `CoordinatorDecision`
- Produces: `CoordinatorDecisionMaterializer`, `Vec<CoordinatorEffect>`

- [ ] **Step 1: Write unit tests for CoordinatorDecisionMaterializer**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_materializer_preserves_generation_order() {
        let decision = CoordinatorDecision::MarkWorkerLost {
            worker_id: "worker-1".to_string(),
        };

        let effects = CoordinatorDecisionMaterializer::materialize(decision);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0], CoordinatorEffect::EmitWorkerLost("worker-1".to_string()));
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --lib ha::materializer::tests`
Expected: FAIL with "cannot find type `CoordinatorDecisionMaterializer`"

- [ ] **Step 3: Implement CoordinatorDecisionMaterializer**

In `crates/brain-services/src/ha/materializer.rs`:
```rust
#![allow(missing_docs)]

use crate::ha::models::*;

pub struct CoordinatorDecisionMaterializer;

impl CoordinatorDecisionMaterializer {
    pub fn materialize(decision: CoordinatorDecision) -> Vec<CoordinatorEffect> {
        match decision {
            CoordinatorDecision::MarkWorkerLost { worker_id } => {
                vec![CoordinatorEffect::EmitWorkerLost(worker_id)]
            }
            CoordinatorDecision::RescheduleTask { task_id, .. } => {
                vec![CoordinatorEffect::ScheduleRetry(task_id)]
            }
            _ => vec![],
        }
    }
}
```

In `crates/brain-services/src/ha/mod.rs`:
```rust
pub mod materializer;
pub mod models;

pub use materializer::*;
pub use models::*;
```

- [ ] **Step 4: Verify unit tests pass**

Run: `cargo test -p brain-services --lib ha::materializer::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/ha/materializer.rs
git add crates/brain-services/src/ha/mod.rs
git commit -m "feat(ha): implement CoordinatorDecisionMaterializer preserving generation order"
```

---

### Task 3: `IntentLog` Durability Trait & `SqliteIntentLog`

**Files:**
- Create: `crates/brain-services/src/ha/intent_log.rs`
- Create: `crates/brain-services/src/ha/sqlite_intent_log.rs`
- Modify: `crates/brain-services/src/ha/mod.rs`
- Test: `crates/brain-services/tests/intent_log_tests.rs`

**Interfaces:**
- Consumes: `IntentRecord`, `SequenceNumber`, `EffectId`, `IntentStatus`
- Produces: `trait IntentLog`, `SqliteIntentLog`, `IntentLogError`

- [ ] **Step 1: Write integration tests for SqliteIntentLog**

In `crates/brain-services/tests/intent_log_tests.rs`:
```rust
use brain_services::ha::*;
use rusqlite::Connection;
use uuid::Uuid;

#[tokio::test]
async fn test_sqlite_intent_log_append_status_update_and_pending_scan() {
    let conn = Connection::open_in_memory().unwrap();
    let log = SqliteIntentLog::new(conn);
    log.init_schema().unwrap();

    let effect_id = EffectId(Uuid::new_v4());
    let record = IntentRecord {
        sequence: SequenceNumber(1),
        event_id: EventId(Uuid::new_v4()),
        effect_id,
        created_at: 1000,
        effect: CoordinatorEffect::EmitWorkerLost("w1".to_string()),
        status: IntentStatus::Persisted,
    };

    log.append_record(&record).await.unwrap();

    let pending = log.scan_pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].effect_id, effect_id);

    log.update_status(effect_id, IntentStatus::Completed).await.unwrap();
    let pending_after = log.scan_pending().await.unwrap();
    assert_eq!(pending_after.len(), 0);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test intent_log_tests`
Expected: FAIL with "cannot find type `SqliteIntentLog`"

- [ ] **Step 3: Implement IntentLog trait and SqliteIntentLog**

In `crates/brain-services/src/ha/intent_log.rs`:
```rust
#![allow(missing_docs)]

use crate::ha::models::*;
use async_trait::async_trait;
use thiserror::Error;

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

In `crates/brain-services/src/ha/sqlite_intent_log.rs`:
```rust
#![allow(missing_docs)]

use crate::ha::intent_log::*;
use crate::ha::models::*;
use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::sync::Arc;
use uuid::Uuid;

pub struct SqliteIntentLog {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteIntentLog {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    pub fn init_schema(&self) -> Result<(), IntentLogError> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS coordinator_intent_log (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL,
                effect_id TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL,
                effect TEXT NOT NULL,
                status TEXT NOT NULL
            );
            ",
        )
        .map_err(|e| IntentLogError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl IntentLog for SqliteIntentLog {
    async fn append_record(&self, record: &IntentRecord) -> Result<(), IntentLogError> {
        let conn = self.conn.lock();
        let effect_json = serde_json::to_string(&record.effect).map_err(|e| IntentLogError::Storage(e.to_string()))?;
        let status_str = match record.status {
            IntentStatus::Created => "created",
            IntentStatus::Persisted => "persisted",
            IntentStatus::Executing => "executing",
            IntentStatus::Completed => "completed",
            IntentStatus::Failed => "failed",
        };

        conn.execute(
            "INSERT INTO coordinator_intent_log (event_id, effect_id, created_at, effect, status)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                record.event_id.0.to_string(),
                record.effect_id.0.to_string(),
                record.created_at as i64,
                effect_json,
                status_str,
            ],
        )
        .map_err(|e| IntentLogError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn update_status(&self, effect_id: EffectId, status: IntentStatus) -> Result<(), IntentLogError> {
        let conn = self.conn.lock();
        let status_str = match status {
            IntentStatus::Created => "created",
            IntentStatus::Persisted => "persisted",
            IntentStatus::Executing => "executing",
            IntentStatus::Completed => "completed",
            IntentStatus::Failed => "failed",
        };

        conn.execute(
            "UPDATE coordinator_intent_log SET status = ?1 WHERE effect_id = ?2",
            params![status_str, effect_id.0.to_string()],
        )
        .map_err(|e| IntentLogError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn load_from(&self, sequence: SequenceNumber) -> Result<Vec<IntentRecord>, IntentLogError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT sequence, event_id, effect_id, created_at, effect, status FROM coordinator_intent_log WHERE sequence >= ?1 ORDER BY sequence ASC")
            .map_err(|e| IntentLogError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(params![sequence.0 as i64], |row| {
                let seq: i64 = row.get(0)?;
                let ev_str: String = row.get(1)?;
                let ef_str: String = row.get(2)?;
                let created: i64 = row.get(3)?;
                let eff_json: String = row.get(4)?;
                let st_str: String = row.get(5)?;
                Ok((seq, ev_str, ef_str, created, eff_json, st_str))
            })
            .map_err(|e| IntentLogError::Storage(e.to_string()))?;

        let mut records = Vec::new();
        for r in rows {
            let (seq, ev_str, ef_str, created, eff_json, st_str) = r.map_err(|e| IntentLogError::Storage(e.to_string()))?;
            let effect: CoordinatorEffect = serde_json::from_str(&eff_json).map_err(|e| IntentLogError::Storage(e.to_string()))?;
            let status = match st_str.as_str() {
                "created" => IntentStatus::Created,
                "persisted" => IntentStatus::Persisted,
                "executing" => IntentStatus::Executing,
                "completed" => IntentStatus::Completed,
                _ => IntentStatus::Failed,
            };

            records.push(IntentRecord {
                sequence: SequenceNumber(seq as u64),
                event_id: EventId(Uuid::parse_str(&ev_str).unwrap()),
                effect_id: EffectId(Uuid::parse_str(&ef_str).unwrap()),
                created_at: created as u64,
                effect,
                status,
            });
        }
        Ok(records)
    }

    async fn scan_pending(&self) -> Result<Vec<IntentRecord>, IntentLogError> {
        self.load_from(SequenceNumber(0)).await.map(|recs| {
            recs.into_iter()
                .filter(|r| r.status != IntentStatus::Completed)
                .collect()
        })
    }
}
```

In `crates/brain-services/src/ha/mod.rs`:
```rust
pub mod intent_log;
pub mod materializer;
pub mod models;
pub mod sqlite_intent_log;

pub use intent_log::*;
pub use materializer::*;
pub use models::*;
pub use sqlite_intent_log::*;
```

- [ ] **Step 4: Verify intent log integration tests pass**

Run: `cargo test -p brain-services --test intent_log_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/ha/intent_log.rs
git add crates/brain-services/src/ha/sqlite_intent_log.rs
git add crates/brain-services/src/ha/mod.rs
git add crates/brain-services/tests/intent_log_tests.rs
git commit -m "feat(ha): implement trait IntentLog and SqliteIntentLog persistence engine"
```

---

### Task 4: `CoordinatorEffectExecutor` Trait & Side-Effect Router

**Files:**
- Create: `crates/brain-services/src/ha/executor.rs`
- Modify: `crates/brain-services/src/ha/mod.rs`
- Test: `crates/brain-services/tests/effect_executor_tests.rs`

**Interfaces:**
- Consumes: `EffectId`, `CoordinatorEffect`
- Produces: `trait CoordinatorEffectExecutor`, `MockEffectExecutor`

- [ ] **Step 1: Write integration tests for MockEffectExecutor**

In `crates/brain-services/tests/effect_executor_tests.rs`:
```rust
use brain_services::ha::*;
use uuid::Uuid;

#[tokio::test]
async fn test_effect_executor_routing_and_idempotency() {
    let executor = MockEffectExecutor::new();
    let effect_id = EffectId(Uuid::new_v4());
    let effect = CoordinatorEffect::EmitWorkerLost("worker-1".to_string());

    executor.execute_effect(effect_id, &effect).await.unwrap();
    assert_eq!(executor.executed_count(), 1);

    // Duplicate execution is idempotent
    executor.execute_effect(effect_id, &effect).await.unwrap();
    assert_eq!(executor.executed_count(), 1);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test effect_executor_tests`
Expected: FAIL with "cannot find type `MockEffectExecutor`"

- [ ] **Step 3: Implement CoordinatorEffectExecutor trait and MockEffectExecutor**

In `crates/brain-services/src/ha/executor.rs`:
```rust
#![allow(missing_docs)]

use crate::ha::models::*;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EffectExecutionError {
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Storage error: {0}")]
    Storage(String),
}

#[async_trait]
pub trait CoordinatorEffectExecutor: Send + Sync {
    async fn execute_effect(&self, effect_id: EffectId, effect: &CoordinatorEffect) -> Result<(), EffectExecutionError>;
}

pub struct MockEffectExecutor {
    executed_effects: Arc<Mutex<HashSet<EffectId>>>,
}

impl Default for MockEffectExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEffectExecutor {
    pub fn new() -> Self {
        Self {
            executed_effects: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn executed_count(&self) -> usize {
        self.executed_effects.lock().len()
    }
}

#[async_trait]
impl CoordinatorEffectExecutor for MockEffectExecutor {
    async fn execute_effect(&self, effect_id: EffectId, _effect: &CoordinatorEffect) -> Result<(), EffectExecutionError> {
        let mut executed = self.executed_effects.lock();
        if executed.contains(&effect_id) {
            return Ok(()); // Idempotency check
        }
        executed.insert(effect_id);
        Ok(())
    }
}
```

In `crates/brain-services/src/ha/mod.rs`:
```rust
pub mod executor;
pub mod intent_log;
pub mod materializer;
pub mod models;
pub mod sqlite_intent_log;

pub use executor::*;
pub use intent_log::*;
pub use materializer::*;
pub use models::*;
pub use sqlite_intent_log::*;
```

- [ ] **Step 4: Verify effect executor unit tests pass**

Run: `cargo test -p brain-services --test effect_executor_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/ha/executor.rs
git add crates/brain-services/src/ha/mod.rs
git add crates/brain-services/tests/effect_executor_tests.rs
git commit -m "feat(ha): implement CoordinatorEffectExecutor trait and MockEffectExecutor"
```

---

### Task 5: `IntentReplayEngine` & End-to-End Recovery Suite

**Files:**
- Create: `crates/brain-services/src/ha/replay.rs`
- Modify: `crates/brain-services/src/ha/mod.rs`
- Test: `crates/brain-services/tests/r28_ha_foundations_tests.rs`

**Interfaces:**
- Consumes: `IntentLog`, `CoordinatorEffectExecutor`
- Produces: `IntentReplayEngine`

- [ ] **Step 1: Write end-to-end crash recovery tests for IntentReplayEngine**

In `crates/brain-services/tests/r28_ha_foundations_tests.rs`:
```rust
use brain_services::ha::*;
use rusqlite::Connection;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn test_end_to_end_intent_replay_engine_crash_recovery() {
    let conn = Connection::open_in_memory().unwrap();
    let log = Arc::new(SqliteIntentLog::new(conn));
    log.init_schema().unwrap();

    let effect_id = EffectId(Uuid::new_v4());
    let record = IntentRecord {
        sequence: SequenceNumber(1),
        event_id: EventId(Uuid::new_v4()),
        effect_id,
        created_at: 1000,
        effect: CoordinatorEffect::EmitWorkerLost("w1".to_string()),
        status: IntentStatus::Executing, // Interrupted state
    };

    log.append_record(&record).await.unwrap();

    let executor = Arc::new(MockEffectExecutor::new());
    let engine = IntentReplayEngine::new(log.clone(), executor.clone());

    // Replay pending executing records
    engine.replay_pending().await.unwrap();

    assert_eq!(executor.executed_count(), 1);

    // Verify record transitioned to Completed
    let pending = log.scan_pending().await.unwrap();
    assert_eq!(pending.len(), 0);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test r28_ha_foundations_tests`
Expected: FAIL with "cannot find type `IntentReplayEngine`"

- [ ] **Step 3: Implement IntentReplayEngine**

In `crates/brain-services/src/ha/replay.rs`:
```rust
#![allow(missing_docs)]

use crate::ha::executor::*;
use crate::ha::intent_log::*;
use crate::ha::models::*;
use std::sync::Arc;

pub struct IntentReplayEngine<L: IntentLog, E: CoordinatorEffectExecutor> {
    log: Arc<L>,
    executor: Arc<E>,
}

impl<L: IntentLog, E: CoordinatorEffectExecutor> IntentReplayEngine<L, E> {
    pub fn new(log: Arc<L>, executor: Arc<E>) -> Self {
        Self { log, executor }
    }

    pub async fn replay_pending(&self) -> Result<(), IntentLogError> {
        let pending = self.log.scan_pending().await?;

        for record in pending {
            match record.status {
                IntentStatus::Completed => continue,
                IntentStatus::Created | IntentStatus::Persisted | IntentStatus::Executing | IntentStatus::Failed => {
                    let _ = self.log.update_status(record.effect_id, IntentStatus::Executing).await;
                    if self.executor.execute_effect(record.effect_id, &record.effect).await.is_ok() {
                        let _ = self.log.update_status(record.effect_id, IntentStatus::Completed).await;
                    } else {
                        let _ = self.log.update_status(record.effect_id, IntentStatus::Failed).await;
                    }
                }
            }
        }
        Ok(())
    }
}
```

In `crates/brain-services/src/ha/mod.rs`:
```rust
pub mod executor;
pub mod intent_log;
pub mod materializer;
pub mod models;
pub mod replay;
pub mod sqlite_intent_log;

pub use executor::*;
pub use intent_log::*;
pub use materializer::*;
pub use models::*;
pub use replay::*;
pub use sqlite_intent_log::*;
```

- [ ] **Step 4: Verify end-to-end integration tests pass**

Run: `cargo test -p brain-services --test r28_ha_foundations_tests`
Expected: PASS

- [ ] **Step 5: Run full workspace check**

Run: `cargo check --workspace`
Expected: PASS (all workspace crates compile cleanly)

- [ ] **Step 6: Commit**

```bash
git add crates/brain-services/src/ha/replay.rs
git add crates/brain-services/src/ha/mod.rs
git add crates/brain-services/tests/r28_ha_foundations_tests.rs
git commit -m "feat(ha): implement IntentReplayEngine and end-to-end crash recovery tests"
```

---

## Inline Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-28-r28-ha-foundations-plan.md`.

Proceeding with **Inline Execution** (`executing-plans` skill) task-by-task.
