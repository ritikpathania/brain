#![allow(missing_docs)]

use crate::runtime::events::*;
use crate::runtime::models::*;
use crate::runtime::repository::*;
use brain_storage::runtime_repository::SqliteExecutionRepository as StorageSqliteExecutionRepository;
use brain_storage::Connection;

pub struct SqliteExecutionRepository {
    inner: StorageSqliteExecutionRepository,
}

impl SqliteExecutionRepository {
    pub fn new(conn: Connection) -> Self {
        Self {
            inner: StorageSqliteExecutionRepository::new(conn),
        }
    }

    pub fn init_schema(&self) -> Result<(), RepositoryError> {
        self.inner.init_schema().map_err(|e| match e {
            brain_storage::RepositoryError::Storage(s) => RepositoryError::Storage(s),
            brain_storage::RepositoryError::ExecutionNotFound(id) => {
                RepositoryError::ExecutionNotFound(ExecutionId(id.0))
            }
        })
    }
}

impl ExecutionRepository for SqliteExecutionRepository {
    fn create_execution(&self, header: &ExecutionHeader) -> Result<(), RepositoryError> {
        let storage_header = brain_storage::runtime_repository::ExecutionHeader {
            execution_id: brain_storage::runtime_repository::ExecutionId(header.execution_id.0),
            parent_execution_id: header
                .parent_execution_id
                .map(|id| brain_storage::runtime_repository::ExecutionId(id.0)),
            root_execution_id: brain_storage::runtime_repository::ExecutionId(
                header.root_execution_id.0,
            ),
            correlation_id: header.correlation_id.clone(),
            cause_id: header.cause_id.clone(),
        };

        use brain_storage::ExecutionRepository as StorageExecutionRepo;
        self.inner
            .create_execution(&storage_header)
            .map_err(|e| match e {
                brain_storage::RepositoryError::Storage(s) => RepositoryError::Storage(s),
                brain_storage::RepositoryError::ExecutionNotFound(id) => {
                    RepositoryError::ExecutionNotFound(ExecutionId(id.0))
                }
            })
    }

    fn get_execution_header(
        &self,
        id: ExecutionId,
    ) -> Result<Option<ExecutionHeader>, RepositoryError> {
        use brain_storage::ExecutionRepository as StorageExecutionRepo;
        let res = self
            .inner
            .get_execution_header(brain_storage::runtime_repository::ExecutionId(id.0))
            .map_err(|e| match e {
                brain_storage::RepositoryError::Storage(s) => RepositoryError::Storage(s),
                brain_storage::RepositoryError::ExecutionNotFound(eid) => {
                    RepositoryError::ExecutionNotFound(ExecutionId(eid.0))
                }
            })?;

        Ok(res.map(|h| ExecutionHeader {
            execution_id: ExecutionId(h.execution_id.0),
            parent_execution_id: h.parent_execution_id.map(|pid| ExecutionId(pid.0)),
            root_execution_id: ExecutionId(h.root_execution_id.0),
            correlation_id: h.correlation_id,
            cause_id: h.cause_id,
        }))
    }

    fn append_journal_event(&self, event: &JournalEvent) -> Result<(), RepositoryError> {
        let payload_json = serde_json::to_string(&event.payload)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let storage_payload: brain_storage::runtime_repository::JournalPayload =
            serde_json::from_str(&payload_json)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        let storage_event = brain_storage::runtime_repository::JournalEvent {
            sequence_no: brain_storage::runtime_repository::SequenceNo(event.sequence_no.0),
            execution_id: brain_storage::runtime_repository::ExecutionId(event.execution_id.0),
            version: brain_storage::runtime_repository::ExecutionVersion(event.version.0),
            occurred_at: event.occurred_at,
            payload: storage_payload,
        };

        use brain_storage::ExecutionRepository as StorageExecutionRepo;
        self.inner
            .append_journal_event(&storage_event)
            .map_err(|e| match e {
                brain_storage::RepositoryError::Storage(s) => RepositoryError::Storage(s),
                brain_storage::RepositoryError::ExecutionNotFound(id) => {
                    RepositoryError::ExecutionNotFound(ExecutionId(id.0))
                }
            })
    }

    fn get_journal_events(
        &self,
        execution_id: ExecutionId,
        after_seq: SequenceNo,
    ) -> Result<Vec<JournalEvent>, RepositoryError> {
        use brain_storage::ExecutionRepository as StorageExecutionRepo;
        let events = self
            .inner
            .get_journal_events(
                brain_storage::runtime_repository::ExecutionId(execution_id.0),
                brain_storage::runtime_repository::SequenceNo(after_seq.0),
            )
            .map_err(|e| match e {
                brain_storage::RepositoryError::Storage(s) => RepositoryError::Storage(s),
                brain_storage::RepositoryError::ExecutionNotFound(id) => {
                    RepositoryError::ExecutionNotFound(ExecutionId(id.0))
                }
            })?;

        let mut result = Vec::new();
        for ev in events {
            let payload_json = serde_json::to_string(&ev.payload)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let payload: JournalPayload = serde_json::from_str(&payload_json)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            result.push(JournalEvent {
                sequence_no: SequenceNo(ev.sequence_no.0),
                execution_id: ExecutionId(ev.execution_id.0),
                version: ExecutionVersion(ev.version.0),
                occurred_at: ev.occurred_at,
                payload,
            });
        }
        Ok(result)
    }
}
