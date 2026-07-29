#![allow(missing_docs)]

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
