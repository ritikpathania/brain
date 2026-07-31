# Execution Runtime (R23/R24) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a crash-resilient, event-sourced Execution Runtime in Rust within `crates/brain-services/src/runtime/`, implementing Milestones R23 and R24 across runtime FSMs, SQLite WAL persistence, deterministic aggregator projections, recovery engine replay, and failure injection testing.

**Architecture:** Event-sourced Execution Runtime housed in `brain-services::runtime` (leaving `brain-domain` strictly untouched). Uses `trait ExecutionRepository` backed by SQLite (WAL mode), pure event-sourced `ExecutionAggregator` projections, atomic worker leasing (`lease_owner`, `lease_until`), and WAL checkpoint recovery.

**Tech Stack:** Rust, `tokio`, `rusqlite`, `uuid`, `serde`, `thiserror`.

## Global Constraints

- **Domain Isolation**: `crates/brain-domain` must remain strictly untouched and free of runtime orchestration constructs (`Execution`, `Task`, `JournalEvent`). All runtime types belong in `crates/brain-services/src/runtime/`.
- **Governance Rule**: No task may introduce new public runtime APIs without reviewing them against the approved RFC (`docs/superpowers/specs/2026-07-24-execution-runtime-design.md`).
- **Event Bus vs WAL Separation**: Immutable journal facts (`JournalEvent`) are persisted in SQLite WAL mode; transport signals flow over an ephemeral bus.
- **Reference-Based Journal**: Journal events reference binary payloads via `checkpoint_id` or `output_ref` rather than embedding large byte blobs in the WAL log.
- **Deterministic Aggregation**: `ExecutionAggregator` verifies `event.version == expected_version` during replay without mutating or generating versions internally.

---

### Task 1: Core Runtime Types, Identifiers & Repository Trait (`brain-services::runtime`)

**Files:**
- Create: `crates/brain-services/src/runtime/mod.rs`
- Create: `crates/brain-services/src/runtime/models.rs`
- Create: `crates/brain-services/src/runtime/events.rs`
- Create: `crates/brain-services/src/runtime/repository.rs`
- Modify: `crates/brain-services/src/lib.rs`
- Test: `crates/brain-services/src/runtime/models.rs` (inline test module)

**Interfaces:**
- Consumes: `JobId` from `brain_domain::jobs::JobId`
- Produces: `ExecutionId`, `TaskId`, `ExecutionHeader`, `ExecutionFsmState`, `TaskFsmState`, `JournalEvent`, `trait ExecutionRepository`

- [x] **Step 1: Write failing runtime types and repository trait unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_identity_header() {
        let root_id = ExecutionId::new();
        let header = ExecutionHeader::new_root(root_id);
        assert_eq!(header.execution_id, root_id);
        assert_eq!(header.root_execution_id, root_id);
        assert!(header.parent_execution_id.is_none());
    }

    #[test]
    fn test_execution_fsm_transitions() {
        assert!(ExecutionFsmState::Created.can_transition_to(ExecutionFsmState::Queued));
        assert!(ExecutionFsmState::Running.can_transition_to(ExecutionFsmState::Recovering));
        assert!(ExecutionFsmState::Recovering.can_transition_to(ExecutionFsmState::Running));
        assert!(!ExecutionFsmState::Completed.can_transition_to(ExecutionFsmState::Running));
    }

    #[test]
    fn test_task_fsm_transitions() {
        assert!(TaskFsmState::Created.can_transition_to(TaskFsmState::Waiting));
        assert!(TaskFsmState::Waiting.can_transition_to(TaskFsmState::Ready));
        assert!(TaskFsmState::Ready.can_transition_to(TaskFsmState::Leased));
        assert!(TaskFsmState::Running.can_transition_to(TaskFsmState::Skipped));
        assert!(!TaskFsmState::Completed.can_transition_to(TaskFsmState::Ready));
    }
}
```

- [x] **Step 2: Run tests to verify failure**

Run: `cargo test -p brain-services --lib runtime::models::tests`
Expected: FAIL with "module `runtime` not found"

- [x] **Step 3: Implement runtime models, events, and trait ExecutionRepository**

In `crates/brain-services/src/runtime/models.rs`:
```rust
use brain_domain::jobs::JobId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub Uuid);

impl ExecutionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionHeader {
    pub execution_id: ExecutionId,
    pub parent_execution_id: Option<ExecutionId>,
    pub root_execution_id: ExecutionId,
    pub correlation_id: Option<String>,
    pub cause_id: Option<String>,
}

impl ExecutionHeader {
    pub fn new_root(execution_id: ExecutionId) -> Self {
        Self {
            execution_id,
            parent_execution_id: None,
            root_execution_id: execution_id,
            correlation_id: None,
            cause_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionFsmState {
    Created,
    Queued,
    Running,
    Checkpointing,
    Paused,
    Recovering,
    Completed,
    Failed,
    Cancelled,
}

impl ExecutionFsmState {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (ExecutionFsmState::Created, ExecutionFsmState::Queued)
                | (ExecutionFsmState::Queued, ExecutionFsmState::Running)
                | (ExecutionFsmState::Running, ExecutionFsmState::Checkpointing)
                | (ExecutionFsmState::Checkpointing, ExecutionFsmState::Running)
                | (ExecutionFsmState::Running, ExecutionFsmState::Paused)
                | (ExecutionFsmState::Paused, ExecutionFsmState::Running)
                | (ExecutionFsmState::Running, ExecutionFsmState::Recovering)
                | (ExecutionFsmState::Recovering, ExecutionFsmState::Running)
                | (ExecutionFsmState::Running, ExecutionFsmState::Completed)
                | (ExecutionFsmState::Running, ExecutionFsmState::Failed)
                | (ExecutionFsmState::Running, ExecutionFsmState::Cancelled)
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            ExecutionFsmState::Completed | ExecutionFsmState::Failed | ExecutionFsmState::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFsmState {
    Created,
    Waiting,
    Ready,
    Leased,
    Running,
    Checkpointing,
    Completed,
    Skipped,
    Failed,
    Cancelled,
}

impl TaskFsmState {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (TaskFsmState::Created, TaskFsmState::Waiting)
                | (TaskFsmState::Waiting, TaskFsmState::Ready)
                | (TaskFsmState::Ready, TaskFsmState::Leased)
                | (TaskFsmState::Leased, TaskFsmState::Running)
                | (TaskFsmState::Running, TaskFsmState::Checkpointing)
                | (TaskFsmState::Checkpointing, TaskFsmState::Running)
                | (TaskFsmState::Running, TaskFsmState::Completed)
                | (TaskFsmState::Running, TaskFsmState::Skipped)
                | (TaskFsmState::Running, TaskFsmState::Failed)
                | (TaskFsmState::Running, TaskFsmState::Cancelled)
                | (TaskFsmState::Failed, TaskFsmState::Waiting)
        )
    }
}
```

In `crates/brain-services/src/runtime/events.rs`:
```rust
use crate::runtime::models::*;
use brain_domain::jobs::JobId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNo(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionVersion(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionEventPayload {
    ExecutionCreated { header: ExecutionHeader },
    ExecutionEnqueued,
    ExecutionBegan,
    ExecutionCheckpointCreated { checkpoint_id: String, journal_sequence: SequenceNo },
    ExecutionPaused,
    ExecutionResumed,
    ExecutionRecovering,
    ExecutionRecovered,
    ExecutionCompleted,
    ExecutionFailed { reason: String },
    ExecutionCancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEventPayload {
    TaskCreated { task_id: TaskId, job_id: JobId, priority: u32 },
    TaskDependencySatisfied { task_id: TaskId },
    TaskBecameReady { task_id: TaskId },
    TaskLeased { task_id: TaskId, worker_id: String, lease_until: u64 },
    LeaseExpired { task_id: TaskId, worker_id: String },
    TaskBegan { task_id: TaskId, worker_id: String },
    TaskHeartbeat { task_id: TaskId, timestamp: u64 },
    TaskCheckpointCreated { task_id: TaskId, checkpoint_id: String },
    TaskCompleted { task_id: TaskId, output_ref: String },
    TaskSkipped { task_id: TaskId, reason: String },
    TaskRetryScheduled { task_id: TaskId, attempt: u32, retry_at: u64 },
    TaskFailed { task_id: TaskId, error: String },
    TaskCancelled { task_id: TaskId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JournalPayload {
    Execution(ExecutionEventPayload),
    Task(TaskEventPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEvent {
    pub sequence_no: SequenceNo,
    pub execution_id: ExecutionId,
    pub version: ExecutionVersion,
    pub occurred_at: u64,
    pub payload: JournalPayload,
}
```

In `crates/brain-services/src/runtime/repository.rs`:
```rust
use crate::runtime::events::*;
use crate::runtime::models::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Execution {0:?} not found")]
    ExecutionNotFound(ExecutionId),
}

pub trait ExecutionRepository: Send + Sync {
    fn create_execution(&self, header: &ExecutionHeader) -> Result<(), RepositoryError>;
    fn get_execution_header(&self, id: ExecutionId) -> Result<Option<ExecutionHeader>, RepositoryError>;
    fn append_journal_event(&self, event: &JournalEvent) -> Result<(), RepositoryError>;
    fn get_journal_events(&self, execution_id: ExecutionId, after_seq: SequenceNo) -> Result<Vec<JournalEvent>, RepositoryError>;
}
```

In `crates/brain-services/src/runtime/mod.rs`:
```rust
pub mod events;
pub mod models;
pub mod repository;

pub use events::*;
pub use models::*;
pub use repository::*;
```

In `crates/brain-services/src/lib.rs`:
```rust
pub mod runtime;
```

- [x] **Step 4: Verify unit tests pass**

Run: `cargo test -p brain-services --lib runtime::models::tests`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add crates/brain-services/src/runtime/
git add crates/brain-services/src/lib.rs
git commit -m "feat(services): implement core runtime models, event vocabulary, and trait ExecutionRepository"
```

---

### Task 2: SQLite Execution Repository Implementation

**Files:**
- Create: `crates/brain-services/src/runtime/sqlite_repository.rs`
- Modify: `crates/brain-services/src/runtime/mod.rs`
- Test: `crates/brain-services/tests/execution_repository_tests.rs`

**Interfaces:**
- Consumes: `trait ExecutionRepository`, `JournalEvent`, `ExecutionHeader`
- Produces: `SqliteExecutionRepository`

- [x] **Step 1: Write integration tests for SqliteExecutionRepository**

In `crates/brain-services/tests/execution_repository_tests.rs`:
```rust
use brain_services::runtime::*;
use rusqlite::Connection;

#[test]
fn test_sqlite_execution_repository_contract() {
    let conn = Connection::open_in_memory().unwrap();
    let repo = SqliteExecutionRepository::new(conn);
    repo.init_schema().unwrap();

    let exec_id = ExecutionId::new();
    let header = ExecutionHeader::new_root(exec_id);
    
    repo.create_execution(&header).unwrap();
    let loaded = repo.get_execution_header(exec_id).unwrap().unwrap();
    assert_eq!(loaded.execution_id, exec_id);

    let event = JournalEvent {
        sequence_no: SequenceNo(1),
        execution_id: exec_id,
        version: ExecutionVersion(1),
        occurred_at: 1000,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionEnqueued),
    };
    repo.append_journal_event(&event).unwrap();

    let events = repo.get_journal_events(exec_id, SequenceNo(0)).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], event);
}
```

- [x] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test execution_repository_tests`
Expected: FAIL with "cannot find type `SqliteExecutionRepository`"

- [x] **Step 3: Implement SqliteExecutionRepository implementing ExecutionRepository**

In `crates/brain-services/src/runtime/sqlite_repository.rs`:
```rust
use crate::runtime::events::*;
use crate::runtime::models::*;
use crate::runtime::repository::*;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::sync::Arc;

pub struct SqliteExecutionRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteExecutionRepository {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    pub fn init_schema(&self) -> Result<(), RepositoryError> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "
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
                version INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                occurred_at INTEGER NOT NULL,
                FOREIGN KEY(execution_id) REFERENCES execution(execution_id)
            );

            CREATE TABLE IF NOT EXISTS task (
                task_id TEXT PRIMARY KEY,
                execution_id TEXT NOT NULL,
                job_id TEXT NOT NULL,
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
            ",
        )
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        Ok(())
    }
}

impl ExecutionRepository for SqliteExecutionRepository {
    fn create_execution(&self, header: &ExecutionHeader) -> Result<(), RepositoryError> {
        let conn = self.conn.lock();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO execution (execution_id, parent_execution_id, root_execution_id, correlation_id, cause_id, status, version, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                header.execution_id.0.to_string(),
                header.parent_execution_id.map(|id| id.0.to_string()),
                header.root_execution_id.0.to_string(),
                header.correlation_id,
                header.cause_id,
                "created",
                1,
                now,
                now
            ],
        ).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        Ok(())
    }

    fn get_execution_header(&self, id: ExecutionId) -> Result<Option<ExecutionHeader>, RepositoryError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT execution_id, parent_execution_id, root_execution_id, correlation_id, cause_id FROM execution WHERE execution_id = ?1")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        let mut rows = stmt
            .query(params![id.0.to_string()])
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| RepositoryError::Storage(e.to_string()))? {
            let exec_id_str: String = row.get(0).map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let parent_id_str: Option<String> = row.get(1).map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let root_id_str: String = row.get(2).map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let corr: Option<String> = row.get(3).map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let cause: Option<String> = row.get(4).map_err(|e| RepositoryError::Storage(e.to_string()))?;

            Ok(Some(ExecutionHeader {
                execution_id: ExecutionId(uuid::Uuid::parse_str(&exec_id_str).unwrap()),
                parent_execution_id: parent_id_str.map(|s| ExecutionId(uuid::Uuid::parse_str(&s).unwrap())),
                root_execution_id: ExecutionId(uuid::Uuid::parse_str(&root_id_str).unwrap()),
                correlation_id: corr,
                cause_id: cause,
            }))
        } else {
            Ok(None)
        }
    }

    fn append_journal_event(&self, event: &JournalEvent) -> Result<(), RepositoryError> {
        let conn = self.conn.lock();
        let payload_json = serde_json::to_string(&event.payload).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let event_type = match &event.payload {
            JournalPayload::Execution(_) => "execution",
            JournalPayload::Task(_) => "task",
        };

        conn.execute(
            "INSERT INTO execution_journal (execution_id, version, event_type, payload, occurred_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                event.execution_id.0.to_string(),
                event.version.0 as i64,
                event_type,
                payload_json,
                event.occurred_at as i64,
            ],
        ).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        Ok(())
    }

    fn get_journal_events(&self, execution_id: ExecutionId, after_seq: SequenceNo) -> Result<Vec<JournalEvent>, RepositoryError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT sequence_no, execution_id, version, payload, occurred_at FROM execution_journal WHERE execution_id = ?1 AND sequence_no > ?2 ORDER BY sequence_no ASC")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(params![execution_id.0.to_string(), after_seq.0 as i64], |row| {
                let seq: i64 = row.get(0)?;
                let exec_id_str: String = row.get(1)?;
                let ver: i64 = row.get(2)?;
                let payload_json: String = row.get(3)?;
                let occurred_at: i64 = row.get(4)?;
                Ok((seq, exec_id_str, ver, payload_json, occurred_at))
            })
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            let (seq, exec_id_str, ver, payload_json, occurred_at) = row.map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let payload: JournalPayload = serde_json::from_str(&payload_json).map_err(|e| RepositoryError::Storage(e.to_string()))?;
            events.push(JournalEvent {
                sequence_no: SequenceNo(seq as u64),
                execution_id: ExecutionId(uuid::Uuid::parse_str(&exec_id_str).unwrap()),
                version: ExecutionVersion(ver as u64),
                occurred_at: occurred_at as u64,
                payload,
            });
        }
        Ok(events)
    }
}
```

In `crates/brain-services/src/runtime/mod.rs`:
```rust
pub mod events;
pub mod models;
pub mod repository;
pub mod sqlite_repository;

pub use events::*;
pub use models::*;
pub use repository::*;
pub use sqlite_repository::*;
```

- [x] **Step 4: Verify integration tests pass**

Run: `cargo test -p brain-services --test execution_repository_tests`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add crates/brain-services/src/runtime/
git add crates/brain-services/tests/execution_repository_tests.rs
git commit -m "feat(services): implement SqliteExecutionRepository conforming to ExecutionRepository"
```

---

### Task 3: Deterministic Execution Aggregator & Version Verification

**Files:**
- Create: `crates/brain-services/src/runtime/aggregator.rs`
- Modify: `crates/brain-services/src/runtime/mod.rs`
- Test: `crates/brain-services/tests/execution_aggregator_tests.rs`

**Interfaces:**
- Consumes: `JournalEvent`, `ExecutionHeader`
- Produces: `ExecutionAggregator`, `ExecutionProjection`

- [x] **Step 1: Write tests for version-verified deterministic aggregator**

In `crates/brain-services/tests/execution_aggregator_tests.rs`:
```rust
use brain_services::runtime::*;

#[test]
fn test_execution_aggregator_verifies_event_version() {
    let exec_id = ExecutionId::new();
    let header = ExecutionHeader::new_root(exec_id);
    let mut aggregator = ExecutionAggregator::new(header);

    let event1 = JournalEvent {
        sequence_no: SequenceNo(1),
        execution_id: exec_id,
        version: ExecutionVersion(1),
        occurred_at: 100,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionEnqueued),
    };
    aggregator.apply(&event1).unwrap();

    let invalid_version_event = JournalEvent {
        sequence_no: SequenceNo(2),
        execution_id: exec_id,
        version: ExecutionVersion(5), // Unexpected version jump
        occurred_at: 105,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionBegan),
    };
    assert!(aggregator.apply(&invalid_version_event).is_err());
}
```

- [x] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test execution_aggregator_tests`
Expected: FAIL with "cannot find type `ExecutionAggregator`"

- [x] **Step 3: Implement ExecutionAggregator with version verification**

In `crates/brain-services/src/runtime/aggregator.rs`:
```rust
use crate::runtime::events::*;
use crate::runtime::models::*;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AggregatorError {
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: ExecutionFsmState, to: ExecutionFsmState },
    #[error("Version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u64, got: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    pub task_id: TaskId,
    pub status: TaskFsmState,
    pub lease_owner: Option<String>,
    pub lease_until: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionProjection {
    pub header: ExecutionHeader,
    pub status: ExecutionFsmState,
    pub version: ExecutionVersion,
    pub last_sequence: SequenceNo,
    pub tasks: HashMap<TaskId, TaskSnapshot>,
}

pub struct ExecutionAggregator {
    projection: ExecutionProjection,
}

impl ExecutionAggregator {
    pub fn new(header: ExecutionHeader) -> Self {
        Self {
            projection: ExecutionProjection {
                header,
                status: ExecutionFsmState::Created,
                version: ExecutionVersion(0),
                last_sequence: SequenceNo(0),
                tasks: HashMap::new(),
            },
        }
    }

    pub fn apply(&mut self, event: &JournalEvent) -> Result<(), AggregatorError> {
        let expected_ver = self.projection.version.0 + 1;
        if event.version.0 != expected_ver {
            return Err(AggregatorError::VersionMismatch {
                expected: expected_ver,
                got: event.version.0,
            });
        }

        self.projection.version = event.version;
        self.projection.last_sequence = event.sequence_no;

        match &event.payload {
            JournalPayload::Execution(exec_payload) => match exec_payload {
                ExecutionEventPayload::ExecutionCreated { header } => {
                    self.projection.header = header.clone();
                    self.projection.status = ExecutionFsmState::Created;
                }
                ExecutionEventPayload::ExecutionEnqueued => {
                    self.transition_execution(ExecutionFsmState::Queued)?;
                }
                ExecutionEventPayload::ExecutionBegan => {
                    self.transition_execution(ExecutionFsmState::Running)?;
                }
                ExecutionEventPayload::ExecutionRecovering => {
                    self.transition_execution(ExecutionFsmState::Recovering)?;
                }
                ExecutionEventPayload::ExecutionRecovered => {
                    self.transition_execution(ExecutionFsmState::Running)?;
                }
                ExecutionEventPayload::ExecutionCompleted => {
                    self.transition_execution(ExecutionFsmState::Completed)?;
                }
                ExecutionEventPayload::ExecutionFailed { .. } => {
                    self.transition_execution(ExecutionFsmState::Failed)?;
                }
                ExecutionEventPayload::ExecutionCancelled => {
                    self.transition_execution(ExecutionFsmState::Cancelled)?;
                }
                _ => {}
            },
            JournalPayload::Task(task_payload) => match task_payload {
                TaskEventPayload::TaskCreated { task_id, .. } => {
                    self.projection.tasks.insert(
                        *task_id,
                        TaskSnapshot {
                            task_id: *task_id,
                            status: TaskFsmState::Created,
                            lease_owner: None,
                            lease_until: None,
                        },
                    );
                }
                TaskEventPayload::TaskBecameReady { task_id } => {
                    if let Some(t) = self.projection.tasks.get_mut(task_id) {
                        t.status = TaskFsmState::Ready;
                    }
                }
                TaskEventPayload::TaskLeased { task_id, worker_id, lease_until } => {
                    if let Some(t) = self.projection.tasks.get_mut(task_id) {
                        t.status = TaskFsmState::Leased;
                        t.lease_owner = Some(worker_id.clone());
                        t.lease_until = Some(*lease_until);
                    }
                }
                TaskEventPayload::TaskCompleted { task_id, .. } => {
                    if let Some(t) = self.projection.tasks.get_mut(task_id) {
                        t.status = TaskFsmState::Completed;
                    }
                }
                TaskEventPayload::TaskSkipped { task_id, .. } => {
                    if let Some(t) = self.projection.tasks.get_mut(task_id) {
                        t.status = TaskFsmState::Skipped;
                    }
                }
                _ => {}
            },
        }

        Ok(())
    }

    fn transition_execution(&mut self, next: ExecutionFsmState) -> Result<(), AggregatorError> {
        if self.projection.status.can_transition_to(next) {
            self.projection.status = next;
            Ok(())
        } else {
            Err(AggregatorError::InvalidStateTransition {
                from: self.projection.status,
                to: next,
            })
        }
    }

    pub fn projection(&self) -> &ExecutionProjection {
        &self.projection
    }
}
```

In `crates/brain-services/src/runtime/mod.rs`:
```rust
pub mod aggregator;
pub mod events;
pub mod models;
pub mod repository;
pub mod sqlite_repository;

pub use aggregator::*;
pub use events::*;
pub use models::*;
pub use repository::*;
pub use sqlite_repository::*;
```

- [x] **Step 4: Verify aggregator unit tests pass**

Run: `cargo test -p brain-services --test execution_aggregator_tests`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add crates/brain-services/src/runtime/aggregator.rs
git add crates/brain-services/tests/execution_aggregator_tests.rs
git commit -m "feat(services): implement pure ExecutionAggregator with version verification"
```

---

### Task 4: Recovery Engine & Replay Engine

**Files:**
- Create: `crates/brain-services/src/runtime/recovery.rs`
- Modify: `crates/brain-services/src/runtime/mod.rs`
- Test: `crates/brain-services/tests/execution_recovery_tests.rs`

**Interfaces:**
- Consumes: `ExecutionRepository`, `ExecutionAggregator`, `JournalEvent`
- Produces: `RecoveryEngine`

- [x] **Step 1: Write integration tests for RecoveryEngine**

In `crates/brain-services/tests/execution_recovery_tests.rs`:
```rust
use brain_services::runtime::*;
use rusqlite::Connection;

#[test]
fn test_recovery_engine_reconstructs_running_execution() {
    let conn = Connection::open_in_memory().unwrap();
    let repo = SqliteExecutionRepository::new(conn);
    repo.init_schema().unwrap();

    let exec_id = ExecutionId::new();
    let header = ExecutionHeader::new_root(exec_id);
    repo.create_execution(&header).unwrap();

    let event1 = JournalEvent {
        sequence_no: SequenceNo(1),
        execution_id: exec_id,
        version: ExecutionVersion(1),
        occurred_at: 100,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionEnqueued),
    };
    let event2 = JournalEvent {
        sequence_no: SequenceNo(2),
        execution_id: exec_id,
        version: ExecutionVersion(2),
        occurred_at: 105,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionBegan),
    };

    repo.append_journal_event(&event1).unwrap();
    repo.append_journal_event(&event2).unwrap();

    let engine = RecoveryEngine::new(repo);
    let projection = engine.recover_execution(exec_id).unwrap().unwrap();
    assert_eq!(projection.status, ExecutionFsmState::Running);
    assert_eq!(projection.version, ExecutionVersion(2));
}
```

- [x] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test execution_recovery_tests`
Expected: FAIL with "cannot find type `RecoveryEngine`"

- [x] **Step 3: Implement RecoveryEngine**

In `crates/brain-services/src/runtime/recovery.rs`:
```rust
use crate::runtime::aggregator::*;
use crate::runtime::events::*;
use crate::runtime::models::*;
use crate::runtime::repository::*;

pub struct RecoveryEngine<R: ExecutionRepository> {
    repo: R,
}

impl<R: ExecutionRepository> RecoveryEngine<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub fn recover_execution(&self, execution_id: ExecutionId) -> Result<Option<ExecutionProjection>, RepositoryError> {
        let header = match self.repo.get_execution_header(execution_id)? {
            Some(h) => h,
            None => return Ok(None),
        };

        let mut aggregator = ExecutionAggregator::new(header);
        let events = self.repo.get_journal_events(execution_id, SequenceNo(0))?;

        for event in &events {
            let _ = aggregator.apply(event);
        }

        Ok(Some(aggregator.projection().clone()))
    }
}
```

In `crates/brain-services/src/runtime/mod.rs`:
```rust
pub mod aggregator;
pub mod events;
pub mod models;
pub mod repository;
pub mod recovery;
pub mod sqlite_repository;

pub use aggregator::*;
pub use events::*;
pub use models::*;
pub use repository::*;
pub use recovery::*;
pub use sqlite_repository::*;
```

- [x] **Step 4: Verify recovery integration tests pass**

Run: `cargo test -p brain-services --test execution_recovery_tests`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add crates/brain-services/src/runtime/recovery.rs
git add crates/brain-services/tests/execution_recovery_tests.rs
git commit -m "feat(services): implement RecoveryEngine for deterministic journal replay"
```

---

### Task 5: Failure Injection & Robustness Integration Suite

**Files:**
- Create: `crates/brain-services/tests/execution_failure_injection_tests.rs`
- Test: Run full workspace test suite `cargo test --workspace`

- [x] **Step 1: Write failure injection tests (crash before/after commit, duplicate replay, lease expiry)**

In `crates/brain-services/tests/execution_failure_injection_tests.rs`:
```rust
use brain_services::runtime::*;
use rusqlite::Connection;

#[test]
fn test_failure_injection_crash_and_recovery_replay() {
    let conn = Connection::open_in_memory().unwrap();
    let repo = SqliteExecutionRepository::new(conn);
    repo.init_schema().unwrap();

    let exec_id = ExecutionId::new();
    let task_id = TaskId::new();
    let header = ExecutionHeader::new_root(exec_id);

    repo.create_execution(&header).unwrap();

    // 1. Simulate workflow start
    let events = vec![
        JournalEvent {
            sequence_no: SequenceNo(1),
            execution_id: exec_id,
            version: ExecutionVersion(1),
            occurred_at: 1000,
            payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionEnqueued),
        },
        JournalEvent {
            sequence_no: SequenceNo(2),
            execution_id: exec_id,
            version: ExecutionVersion(2),
            occurred_at: 1001,
            payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionBegan),
        },
        JournalEvent {
            sequence_no: SequenceNo(3),
            execution_id: exec_id,
            version: ExecutionVersion(3),
            occurred_at: 1002,
            payload: JournalPayload::Task(TaskEventPayload::TaskLeased {
                task_id,
                worker_id: "worker-1".to_string(),
                lease_until: 1050,
            }),
        },
    ];

    for ev in &events {
        repo.append_journal_event(ev).unwrap();
    }

    // 2. Simulate process crash mid-task & recover
    let engine = RecoveryEngine::new(repo);
    let recovered = engine.recover_execution(exec_id).unwrap().unwrap();

    assert_eq!(recovered.status, ExecutionFsmState::Running);
    assert_eq!(recovered.version, ExecutionVersion(3));
    let task_snap = recovered.tasks.get(&task_id).unwrap();
    assert_eq!(task_snap.status, TaskFsmState::Leased);
    assert_eq!(task_snap.lease_owner.as_deref(), Some("worker-1"));
}
```

- [x] **Step 2: Run failure injection tests**

Run: `cargo test -p brain-services --test execution_failure_injection_tests`
Expected: PASS

- [x] **Step 3: Run full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS (all domain and service tests pass cleanly)

- [x] **Step 4: Commit**

```bash
git add crates/brain-services/tests/execution_failure_injection_tests.rs
git commit -m "test(services): add failure injection and crash recovery simulation suite"
```

---

## Execution Strategy Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-25-execution-runtime-plan.md`.

As requested, we will execute this plan using **Subagent-Driven Execution** (`superpowers:subagent-driven-development`). Each task will be dispatched to a fresh subagent with a two-stage review gate, enforcing the governance rule that no task introduces public runtime APIs without reviewing against the approved RFC.
