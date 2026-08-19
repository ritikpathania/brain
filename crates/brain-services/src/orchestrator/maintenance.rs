use brain_core::errors::BrainError;
use brain_storage::connection::SqliteConnectionManager;
use brain_storage::r2d2::Pool;
use std::sync::Arc;

/// Engine executing periodic and opportunistic SQLite database maintenance operations.
pub struct MaintenanceEngine {
    pool: Arc<Pool<SqliteConnectionManager>>,
}

impl MaintenanceEngine {
    /// Creates a new `MaintenanceEngine`.
    pub fn new(pool: Arc<Pool<SqliteConnectionManager>>) -> Self {
        Self { pool }
    }

    /// Performs passive WAL checkpointing (`PRAGMA wal_checkpoint(PASSIVE)`).
    pub fn checkpoint_wal(&self) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to acquire connection for WAL checkpoint: {:?}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute("PRAGMA wal_checkpoint(PASSIVE);", [])
            .map_err(|e| BrainError::Storage {
                message: format!("WAL checkpoint failed: {:?}", e),
                source: Some(Box::new(e)),
            })?;

        Ok(())
    }

    /// Performs database optimization and incremental vacuum during idle windows.
    pub fn vacuum(&self) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to acquire connection for vacuum: {:?}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute("PRAGMA optimize;", [])
            .map_err(|e| BrainError::Storage {
                message: format!("PRAGMA optimize failed: {:?}", e),
                source: Some(Box::new(e)),
            })?;

        Ok(())
    }
}
