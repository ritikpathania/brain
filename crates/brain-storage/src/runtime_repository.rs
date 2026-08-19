//! Runtime execution repository and persistent SQLite storage.

#![allow(missing_docs)]

use brain_domain::jobs::JobId;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub Uuid);

impl ExecutionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNo(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionVersion(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionEventPayload {
    ExecutionCreated {
        header: ExecutionHeader,
    },
    ExecutionEnqueued,
    ExecutionBegan,
    ExecutionCheckpointCreated {
        checkpoint_id: String,
        journal_sequence: SequenceNo,
    },
    ExecutionPaused,
    ExecutionResumed,
    ExecutionRecovering,
    ExecutionRecovered,
    ExecutionCompleted,
    ExecutionFailed {
        reason: String,
    },
    ExecutionCancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEventPayload {
    TaskCreated {
        task_id: TaskId,
        job_id: JobId,
        priority: u32,
    },
    TaskDependencySatisfied {
        task_id: TaskId,
    },
    TaskBecameReady {
        task_id: TaskId,
    },
    TaskLeased {
        task_id: TaskId,
        worker_id: String,
        lease_until: u64,
    },
    LeaseExpired {
        task_id: TaskId,
        worker_id: String,
    },
    TaskBegan {
        task_id: TaskId,
        worker_id: String,
    },
    TaskHeartbeat {
        task_id: TaskId,
        timestamp: u64,
    },
    TaskCheckpointCreated {
        task_id: TaskId,
        checkpoint_id: String,
    },
    TaskCompleted {
        task_id: TaskId,
        output_ref: String,
    },
    TaskSkipped {
        task_id: TaskId,
        reason: String,
    },
    TaskRetryScheduled {
        task_id: TaskId,
        attempt: u32,
        retry_at: u64,
    },
    TaskFailed {
        task_id: TaskId,
        error: String,
    },
    TaskCancelled {
        task_id: TaskId,
    },
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

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Execution {0:?} not found")]
    ExecutionNotFound(ExecutionId),
}

pub trait ExecutionRepository: Send + Sync {
    fn create_execution(&self, header: &ExecutionHeader) -> Result<(), RepositoryError>;
    fn get_execution_header(
        &self,
        id: ExecutionId,
    ) -> Result<Option<ExecutionHeader>, RepositoryError>;
    fn append_journal_event(&self, event: &JournalEvent) -> Result<(), RepositoryError>;
    fn get_journal_events(
        &self,
        execution_id: ExecutionId,
        after_seq: SequenceNo,
    ) -> Result<Vec<JournalEvent>, RepositoryError>;
}

pub struct SqliteExecutionRepository {
    conn: Arc<std::sync::Mutex<Connection>>,
}

impl SqliteExecutionRepository {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(std::sync::Mutex::new(conn)),
        }
    }

    pub fn init_schema(&self) -> Result<(), RepositoryError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
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

    fn get_execution_header(
        &self,
        id: ExecutionId,
    ) -> Result<Option<ExecutionHeader>, RepositoryError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT execution_id, parent_execution_id, root_execution_id, correlation_id, cause_id FROM execution WHERE execution_id = ?1")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        let mut rows = stmt
            .query(params![id.0.to_string()])
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| RepositoryError::Storage(e.to_string()))?
        {
            let exec_id_str: String = row
                .get(0)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let parent_id_str: Option<String> = row
                .get(1)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let root_id_str: String = row
                .get(2)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let corr: Option<String> = row
                .get(3)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let cause: Option<String> = row
                .get(4)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            Ok(Some(ExecutionHeader {
                execution_id: ExecutionId(uuid::Uuid::parse_str(&exec_id_str).unwrap()),
                parent_execution_id: parent_id_str
                    .map(|s| ExecutionId(uuid::Uuid::parse_str(&s).unwrap())),
                root_execution_id: ExecutionId(uuid::Uuid::parse_str(&root_id_str).unwrap()),
                correlation_id: corr,
                cause_id: cause,
            }))
        } else {
            Ok(None)
        }
    }

    fn append_journal_event(&self, event: &JournalEvent) -> Result<(), RepositoryError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let payload_json = serde_json::to_string(&event.payload)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
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

    fn get_journal_events(
        &self,
        execution_id: ExecutionId,
        after_seq: SequenceNo,
    ) -> Result<Vec<JournalEvent>, RepositoryError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT sequence_no, execution_id, version, payload, occurred_at FROM execution_journal WHERE execution_id = ?1 AND sequence_no > ?2 ORDER BY sequence_no ASC")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(
                params![execution_id.0.to_string(), after_seq.0 as i64],
                |row| {
                    let seq: i64 = row.get(0)?;
                    let exec_id_str: String = row.get(1)?;
                    let ver: i64 = row.get(2)?;
                    let payload_json: String = row.get(3)?;
                    let occurred_at: i64 = row.get(4)?;
                    Ok((seq, exec_id_str, ver, payload_json, occurred_at))
                },
            )
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            let (seq, exec_id_str, ver, payload_json, occurred_at) =
                row.map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let payload: JournalPayload = serde_json::from_str(&payload_json)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
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
