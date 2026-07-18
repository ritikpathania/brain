//! Storage repository and model for the database event log.

use brain_core::errors::BrainError;
use rusqlite::params;
use uuid::Uuid;

/// Database model representing a row in the system event log.
#[derive(Debug, Clone)]
pub struct StoredEvent {
    /// Database-assigned chronological sequence number.
    pub sequence: u64,
    /// Unique event identifier.
    pub event_id: Uuid,
    /// Correlation identifier for request tracing.
    pub correlation_id: Uuid,
    /// Unix timestamp when the event was fired.
    pub timestamp_ms: u64,
    /// Payload envelope version.
    pub version: String,
    /// System or service origin node that published the event.
    pub source: String,
    /// Event category/topic.
    pub topic: String,
    /// Serialized event payload.
    pub payload_json: String,
}

/// SQLite database event log backend wrapping a shared connection pool.
pub struct SqliteEventLog {
    pool: r2d2::Pool<crate::connection::SqliteConnectionManager>,
}

impl SqliteEventLog {
    /// Creates a new `SqliteEventLog` instance.
    pub fn new(pool: r2d2::Pool<crate::connection::SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    /// Appends a raw event to the database event log.
    #[allow(clippy::too_many_arguments)]
    pub fn append(
        &self,
        event_id: Uuid,
        correlation_id: Uuid,
        timestamp_ms: u64,
        version: &str,
        source: &str,
        topic: &str,
        payload_json: &str,
    ) -> Result<u64, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute(
            "INSERT INTO system_event_log (event_id, correlation_id, timestamp_ms, version, source, topic, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event_id.to_string(),
                correlation_id.to_string(),
                timestamp_ms as i64,
                version,
                source,
                topic,
                payload_json,
            ],
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to insert system event: {}", e),
            source: Some(Box::new(e)),
        })?;

        let row_id = conn.last_insert_rowid();
        Ok(row_id as u64)
    }

    /// Reads events from the log starting at a specific sequence ID.
    pub fn read_from(
        &self,
        start_sequence: u64,
        limit: usize,
    ) -> Result<Vec<StoredEvent>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn.prepare(
            "SELECT sequence, event_id, correlation_id, timestamp_ms, version, source, topic, payload
             FROM system_event_log
             WHERE sequence >= ?1
             ORDER BY sequence ASC
             LIMIT ?2"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare select statement: {}", e),
            source: Some(Box::new(e)),
        })?;

        let rows = stmt
            .query_map(params![start_sequence as i64, limit as i64], |row| {
                let sequence: i64 = row.get(0)?;
                let event_id_str: String = row.get(1)?;
                let correlation_id_str: String = row.get(2)?;
                let timestamp_ms: i64 = row.get(3)?;
                let version: String = row.get(4)?;
                let source: String = row.get(5)?;
                let topic: String = row.get(6)?;
                let payload_json: String = row.get(7)?;

                let event_id = Uuid::parse_str(&event_id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let correlation_id = Uuid::parse_str(&correlation_id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                Ok(StoredEvent {
                    sequence: sequence as u64,
                    event_id,
                    correlation_id,
                    timestamp_ms: timestamp_ms as u64,
                    version,
                    source,
                    topic,
                    payload_json,
                })
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to query system event log: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut results = Vec::new();
        for row in rows {
            let ev = row.map_err(|e| BrainError::Storage {
                message: format!("Failed to parse system event row: {}", e),
                source: Some(Box::new(e)),
            })?;
            results.push(ev);
        }

        Ok(results)
    }

    /// Retrieves the latest sequence number in the log.
    pub fn latest_sequence(&self) -> Result<u64, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let seq_opt: Option<i64> = conn
            .query_row("SELECT MAX(sequence) FROM system_event_log", [], |row| {
                row.get(0)
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to query max sequence: {}", e),
                source: Some(Box::new(e)),
            })?;

        Ok(seq_opt.unwrap_or(0) as u64)
    }
}

/// Trait defining atomic CRUD and deduplication operations for the ingestion Write-Ahead Event Log.
pub trait EventLogRepository: Send + Sync {
    /// Inserts an ingestion event into the event_log table.
    /// Performs deduplication by checking event_id. If duplicate, returns Ok(existing_sequence).
    fn insert_event(
        &self,
        envelope: &brain_integrations::IngestionEnvelope,
    ) -> Result<u64, BrainError>;

    /// Checks if the event_id already exists in the log.
    fn is_duplicate_event(&self, event_id: &brain_domain::EventId) -> Result<bool, BrainError>;

    /// Replays events starting after the given sequence number.
    fn get_events_after(
        &self,
        sequence: u64,
    ) -> Result<Vec<brain_integrations::IngestionEnvelope>, BrainError>;
}
