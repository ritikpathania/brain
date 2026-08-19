//! HA Coordinator Intent Log persistent SQLite storage.

#![allow(missing_docs)]

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
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
pub struct RawIntentRecord {
    pub sequence: SequenceNumber,
    pub event_id: EventId,
    pub effect_id: EffectId,
    pub created_at: u64,
    pub effect_json: String,
    pub status: IntentStatus,
}

#[derive(Debug, Error)]
pub enum IntentLogError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Duplicate sequence number {0:?}")]
    DuplicateSequence(SequenceNumber),
}

pub struct SqliteIntentLog {
    conn: Arc<std::sync::Mutex<Connection>>,
}

impl SqliteIntentLog {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(std::sync::Mutex::new(conn)),
        }
    }

    pub fn init_schema(&self) -> Result<(), IntentLogError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IntentLogError::Storage(e.to_string()))?;
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

    pub fn append_record_raw(&self, record: &RawIntentRecord) -> Result<(), IntentLogError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IntentLogError::Storage(e.to_string()))?;
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
                record.effect_json,
                status_str,
            ],
        )
        .map_err(|e| IntentLogError::Storage(e.to_string()))?;
        Ok(())
    }

    pub fn update_status_raw(
        &self,
        effect_id: EffectId,
        status: IntentStatus,
    ) -> Result<(), IntentLogError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IntentLogError::Storage(e.to_string()))?;
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

    pub fn load_from_raw(
        &self,
        sequence: SequenceNumber,
    ) -> Result<Vec<RawIntentRecord>, IntentLogError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IntentLogError::Storage(e.to_string()))?;
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
            let (seq, ev_str, ef_str, created, eff_json, st_str) =
                r.map_err(|e| IntentLogError::Storage(e.to_string()))?;
            let status = match st_str.as_str() {
                "created" => IntentStatus::Created,
                "persisted" => IntentStatus::Persisted,
                "executing" => IntentStatus::Executing,
                "completed" => IntentStatus::Completed,
                _ => IntentStatus::Failed,
            };

            records.push(RawIntentRecord {
                sequence: SequenceNumber(seq as u64),
                event_id: EventId(Uuid::parse_str(&ev_str).unwrap()),
                effect_id: EffectId(Uuid::parse_str(&ef_str).unwrap()),
                created_at: created as u64,
                effect_json: eff_json,
                status,
            });
        }
        Ok(records)
    }
}
