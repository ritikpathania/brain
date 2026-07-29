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
    async fn update_status(
        &self,
        effect_id: EffectId,
        status: IntentStatus,
    ) -> Result<(), IntentLogError>;
    async fn load_from(
        &self,
        sequence: SequenceNumber,
    ) -> Result<Vec<IntentRecord>, IntentLogError>;
    async fn scan_pending(&self) -> Result<Vec<IntentRecord>, IntentLogError>;
}
