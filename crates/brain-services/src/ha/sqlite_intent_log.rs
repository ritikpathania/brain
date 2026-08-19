#![allow(missing_docs)]

use crate::ha::intent_log::*;
use crate::ha::models::*;
use async_trait::async_trait;
use brain_storage::intent_log::{RawIntentRecord, SqliteIntentLog as StorageSqliteIntentLog};
use brain_storage::Connection;

pub struct SqliteIntentLog {
    inner: StorageSqliteIntentLog,
}

impl SqliteIntentLog {
    pub fn new(conn: Connection) -> Self {
        Self {
            inner: StorageSqliteIntentLog::new(conn),
        }
    }

    pub fn init_schema(&self) -> Result<(), IntentLogError> {
        self.inner
            .init_schema()
            .map_err(|e| IntentLogError::Storage(e.to_string()))
    }
}

#[async_trait]
impl IntentLog for SqliteIntentLog {
    async fn append_record(&self, record: &IntentRecord) -> Result<(), IntentLogError> {
        let effect_json = serde_json::to_string(&record.effect)
            .map_err(|e| IntentLogError::Storage(e.to_string()))?;

        let raw = RawIntentRecord {
            sequence: brain_storage::intent_log::SequenceNumber(record.sequence.0),
            event_id: brain_storage::intent_log::EventId(record.event_id.0),
            effect_id: brain_storage::intent_log::EffectId(record.effect_id.0),
            created_at: record.created_at,
            effect_json,
            status: match record.status {
                IntentStatus::Created => brain_storage::intent_log::IntentStatus::Created,
                IntentStatus::Persisted => brain_storage::intent_log::IntentStatus::Persisted,
                IntentStatus::Executing => brain_storage::intent_log::IntentStatus::Executing,
                IntentStatus::Completed => brain_storage::intent_log::IntentStatus::Completed,
                IntentStatus::Failed => brain_storage::intent_log::IntentStatus::Failed,
            },
        };

        self.inner
            .append_record_raw(&raw)
            .map_err(|e| IntentLogError::Storage(e.to_string()))
    }

    async fn update_status(
        &self,
        effect_id: EffectId,
        status: IntentStatus,
    ) -> Result<(), IntentLogError> {
        let storage_status = match status {
            IntentStatus::Created => brain_storage::intent_log::IntentStatus::Created,
            IntentStatus::Persisted => brain_storage::intent_log::IntentStatus::Persisted,
            IntentStatus::Executing => brain_storage::intent_log::IntentStatus::Executing,
            IntentStatus::Completed => brain_storage::intent_log::IntentStatus::Completed,
            IntentStatus::Failed => brain_storage::intent_log::IntentStatus::Failed,
        };

        self.inner
            .update_status_raw(
                brain_storage::intent_log::EffectId(effect_id.0),
                storage_status,
            )
            .map_err(|e| IntentLogError::Storage(e.to_string()))
    }

    async fn load_from(
        &self,
        sequence: SequenceNumber,
    ) -> Result<Vec<IntentRecord>, IntentLogError> {
        let raw_records = self
            .inner
            .load_from_raw(brain_storage::intent_log::SequenceNumber(sequence.0))
            .map_err(|e| IntentLogError::Storage(e.to_string()))?;

        let mut records = Vec::new();
        for r in raw_records {
            let effect: CoordinatorEffect = serde_json::from_str(&r.effect_json)
                .map_err(|e| IntentLogError::Storage(e.to_string()))?;
            let status = match r.status {
                brain_storage::intent_log::IntentStatus::Created => IntentStatus::Created,
                brain_storage::intent_log::IntentStatus::Persisted => IntentStatus::Persisted,
                brain_storage::intent_log::IntentStatus::Executing => IntentStatus::Executing,
                brain_storage::intent_log::IntentStatus::Completed => IntentStatus::Completed,
                brain_storage::intent_log::IntentStatus::Failed => IntentStatus::Failed,
            };

            records.push(IntentRecord {
                sequence: SequenceNumber(r.sequence.0),
                event_id: EventId(r.event_id.0),
                effect_id: EffectId(r.effect_id.0),
                created_at: r.created_at,
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
