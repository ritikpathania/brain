//! SQLite-backed repository for tracking stateful projection checkpoints.

use rusqlite::params;
use brain_core::errors::BrainError;

/// SQLite implementation for persisting projection checkpoints.
pub struct SqliteProjectionCheckpointRepository {
    pool: r2d2::Pool<crate::connection::SqliteConnectionManager>,
}

impl SqliteProjectionCheckpointRepository {
    /// Creates a new checkpoint repository.
    pub fn new(pool: r2d2::Pool<crate::connection::SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    /// Retrieves the last successfully processed sequence ID for a projection. Returns 0 if none exists.
    pub fn get_checkpoint(&self, name: &str) -> Result<u64, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let res: Result<i64, rusqlite::Error> = conn.query_row(
            "SELECT last_sequence FROM projection_checkpoints WHERE projection_name = ?1",
            params![name],
            |row| row.get(0),
        );

        match res {
            Ok(seq) => Ok(seq as u64),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(BrainError::Storage {
                message: format!("Failed to query projection checkpoint: {}", e),
                source: Some(Box::new(e)),
            }),
        }
    }

    /// Saves the checkpoint sequence number for a projection.
    pub fn save_checkpoint(&self, name: &str, sequence: u64) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute(
            "INSERT INTO projection_checkpoints (projection_name, last_sequence)
             VALUES (?1, ?2)
             ON CONFLICT(projection_name) DO UPDATE SET last_sequence = excluded.last_sequence",
            params![name, sequence as i64],
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to save projection checkpoint: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }
}
