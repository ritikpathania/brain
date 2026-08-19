//! Persistent SQLite Planning Event Log Storage Implementation.

#![allow(missing_docs)]

use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNumber(pub u64);

impl std::fmt::Display for SequenceNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "seq_{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope<E> {
    pub sequence: SequenceNumber,
    pub timestamp_ms: u64,
    pub schema_version: u16,
    pub payload: E,
}

#[derive(Debug, Error)]
pub enum EventPublishError {
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
}

pub trait EventCodec<E>: Send + Sync {
    fn encode(&self, event: &E) -> Result<Vec<u8>, EventPublishError>;
    fn decode(&self, bytes: &[u8]) -> Result<E, EventPublishError>;
}

pub trait EventLog<E>: Send + Sync {
    fn append(
        &self,
        event: E,
        timestamp_ms: u64,
        schema_version: u16,
    ) -> Result<SequenceNumber, EventPublishError>;

    fn read_range(
        &self,
        start: SequenceNumber,
        limit: usize,
    ) -> Result<Vec<EventEnvelope<E>>, EventPublishError>;

    fn last_sequence_number(&self) -> SequenceNumber;
}

pub struct SqliteEventLog<E, C> {
    conn: Mutex<rusqlite::Connection>,
    codec: C,
    _marker: std::marker::PhantomData<E>,
}

impl<E, C: EventCodec<E>> SqliteEventLog<E, C> {
    pub fn new(path: &str, codec: C) -> Result<Self, EventPublishError> {
        let conn = if path == ":memory:" {
            rusqlite::Connection::open_in_memory()
        } else {
            rusqlite::Connection::open(path)
        }
        .map_err(|e| EventPublishError::StorageError(format!("SQLite open error: {}", e)))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS control_plane_event_log (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_ms INTEGER NOT NULL,
                schema_version INTEGER NOT NULL,
                payload BLOB NOT NULL
            )",
            [],
        )
        .map_err(|e| EventPublishError::StorageError(format!("SQLite schema init error: {}", e)))?;

        Ok(Self {
            conn: Mutex::new(conn),
            codec,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<E: Send + Sync, C: EventCodec<E>> EventLog<E> for SqliteEventLog<E, C> {
    fn append(
        &self,
        event: E,
        timestamp_ms: u64,
        schema_version: u16,
    ) -> Result<SequenceNumber, EventPublishError> {
        let payload_bytes = self.codec.encode(&event)?;
        let mut guard = self
            .conn
            .lock()
            .map_err(|e| EventPublishError::StorageError(format!("Lock poisoning error: {}", e)))?;

        let tx = guard.transaction().map_err(|e| {
            EventPublishError::StorageError(format!("SQLite transaction begin error: {}", e))
        })?;

        tx.execute(
            "INSERT INTO control_plane_event_log (timestamp_ms, schema_version, payload) VALUES (?1, ?2, ?3)",
            params![timestamp_ms as i64, schema_version as i32, payload_bytes],
        )
        .map_err(|e| EventPublishError::StorageError(format!("SQLite insert error: {}", e)))?;

        let seq_val = tx.last_insert_rowid() as u64;

        tx.commit().map_err(|e| {
            EventPublishError::StorageError(format!("SQLite transaction commit error: {}", e))
        })?;

        Ok(SequenceNumber(seq_val))
    }

    fn read_range(
        &self,
        start: SequenceNumber,
        limit: usize,
    ) -> Result<Vec<EventEnvelope<E>>, EventPublishError> {
        if start.0 == 0 || limit == 0 {
            return Ok(Vec::new());
        }

        let guard = self
            .conn
            .lock()
            .map_err(|e| EventPublishError::StorageError(format!("Lock poisoning error: {}", e)))?;

        let mut stmt = guard
            .prepare(
                "SELECT sequence, timestamp_ms, schema_version, payload FROM control_plane_event_log WHERE sequence >= ?1 ORDER BY sequence ASC LIMIT ?2",
            )
            .map_err(|e| EventPublishError::StorageError(format!("SQLite prepare query error: {}", e)))?;

        let rows = stmt
            .query_map(params![start.0 as i64, limit as i64], |row| {
                let seq: i64 = row.get(0)?;
                let ts: i64 = row.get(1)?;
                let ver: i32 = row.get(2)?;
                let bytes: Vec<u8> = row.get(3)?;
                Ok((seq as u64, ts as u64, ver as u16, bytes))
            })
            .map_err(|e| {
                EventPublishError::StorageError(format!("SQLite query map error: {}", e))
            })?;

        let mut envelopes = Vec::new();
        for row in rows {
            let (seq, ts, ver, bytes) = row.map_err(|e| {
                EventPublishError::StorageError(format!("SQLite row fetch error: {}", e))
            })?;
            let payload = self.codec.decode(&bytes)?;
            envelopes.push(EventEnvelope {
                sequence: SequenceNumber(seq),
                timestamp_ms: ts,
                schema_version: ver,
                payload,
            });
        }

        Ok(envelopes)
    }

    fn last_sequence_number(&self) -> SequenceNumber {
        let guard = match self.conn.lock() {
            Ok(g) => g,
            Err(_) => return SequenceNumber(0),
        };

        let mut stmt = match guard.prepare("SELECT MAX(sequence) FROM control_plane_event_log") {
            Ok(s) => s,
            Err(_) => return SequenceNumber(0),
        };

        let seq_opt: Option<i64> = stmt.query_row([], |row| row.get(0)).unwrap_or(None);
        SequenceNumber(seq_opt.unwrap_or(0) as u64)
    }
}
