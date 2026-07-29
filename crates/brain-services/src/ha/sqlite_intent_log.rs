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
        let effect_json = serde_json::to_string(&record.effect)
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
                effect_json,
                status_str,
            ],
        )
        .map_err(|e| IntentLogError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn update_status(
        &self,
        effect_id: EffectId,
        status: IntentStatus,
    ) -> Result<(), IntentLogError> {
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

    async fn load_from(
        &self,
        sequence: SequenceNumber,
    ) -> Result<Vec<IntentRecord>, IntentLogError> {
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
            let (seq, ev_str, ef_str, created, eff_json, st_str) =
                r.map_err(|e| IntentLogError::Storage(e.to_string()))?;
            let effect: CoordinatorEffect = serde_json::from_str(&eff_json)
                .map_err(|e| IntentLogError::Storage(e.to_string()))?;
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
