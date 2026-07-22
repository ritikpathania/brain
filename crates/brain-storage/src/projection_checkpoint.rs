//! SQLite-backed repository for tracking stateful projection checkpoints and metadata.

use brain_core::errors::BrainError;
use rusqlite::params;

/// Strongly-typed projection status to represent the health/lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionStatus {
    /// Projection is idle and ready for ticks.
    Idle,
    /// Projection is actively running catch-up processing.
    Active,
    /// Projection is performing a complete rebuild/replay.
    Rebuilding,
    /// Projection execution failed due to an error.
    Failed,
}

/// Durable metadata record for a registered projection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ProjectionMetadataRecord {
    /// Unique name/ID of the projection.
    pub name: String,
    /// Current logical schema/code version of the projection.
    pub version: u32,
    /// Latest successfully processed event sequence number.
    pub last_sequence: u64,
    /// Current execution state/health status.
    pub status: ProjectionStatus,
    /// String representation of the last encountered error, if failed.
    pub last_error: Option<String>,
    /// Epoch timestamp of the last metadata update in seconds.
    pub updated_at: u64,
}

/// SQLite implementation for persisting and retrieving projection metadata.
/// Stateless repository following Option A: receives an active database connection
/// from the caller and does not manage connection pools itself.
#[derive(Default)]
pub struct SqliteProjectionMetadataRepository;

impl SqliteProjectionMetadataRepository {
    /// Creates a new metadata repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Retrieves projection metadata using an active database connection.
    pub fn get_metadata(
        &self,
        conn: &rusqlite::Connection,
        name: &str,
    ) -> Result<Option<ProjectionMetadataRecord>, BrainError> {
        let mut stmt = conn.prepare(
            "SELECT projection_name, projection_version, last_sequence, status, last_error, updated_at 
             FROM projection_metadata WHERE projection_name = ?1"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare select query: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut rows = stmt.query(params![name]).map_err(|e| BrainError::Storage {
            message: format!("Failed to query projection metadata: {}", e),
            source: Some(Box::new(e)),
        })?;

        if let Some(row) = rows.next().map_err(|e| BrainError::Storage {
            message: format!("Failed to get next row: {}", e),
            source: Some(Box::new(e)),
        })? {
            let status_str: String = row.get(3).map_err(|e| BrainError::Storage {
                message: format!("Failed to get status: {}", e),
                source: Some(Box::new(e)),
            })?;
            let status = match status_str.as_str() {
                "idle" => ProjectionStatus::Idle,
                "active" => ProjectionStatus::Active,
                "rebuilding" => ProjectionStatus::Rebuilding,
                "failed" => ProjectionStatus::Failed,
                _ => ProjectionStatus::Idle,
            };

            Ok(Some(ProjectionMetadataRecord {
                name: row.get(0).unwrap(),
                version: row.get(1).unwrap(),
                last_sequence: row.get(2).unwrap(),
                status,
                last_error: row.get(4).unwrap(),
                updated_at: row.get(5).unwrap(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Saves projection metadata using an active database connection.
    pub fn save_metadata(
        &self,
        conn: &rusqlite::Connection,
        record: &ProjectionMetadataRecord,
    ) -> Result<(), BrainError> {
        let status_str = match record.status {
            ProjectionStatus::Idle => "idle",
            ProjectionStatus::Active => "active",
            ProjectionStatus::Rebuilding => "rebuilding",
            ProjectionStatus::Failed => "failed",
        };

        conn.execute(
            "INSERT INTO projection_metadata (projection_name, projection_version, last_sequence, status, last_error, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(projection_name) DO UPDATE SET
                 projection_version = excluded.projection_version,
                 last_sequence = excluded.last_sequence,
                 status = excluded.status,
                 last_error = excluded.last_error,
                 updated_at = excluded.updated_at",
            params![
                record.name,
                record.version,
                record.last_sequence as i64,
                status_str,
                record.last_error,
                record.updated_at as i64,
            ],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to save projection metadata: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }
}

/// SQLite implementation for persisting projection checkpoints.
/// Kept for backward compatibility with existing tests and dynamic runners.
pub struct SqliteProjectionCheckpointRepository {
    pool: r2d2::Pool<crate::connection::SqliteConnectionManager>,
}

impl SqliteProjectionCheckpointRepository {
    /// Creates a new checkpoint repository.
    pub fn new(pool: r2d2::Pool<crate::connection::SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    /// Exposes the connection pool.
    pub fn pool(&self) -> &r2d2::Pool<crate::connection::SqliteConnectionManager> {
        &self.pool
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
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to save projection checkpoint: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }
}
