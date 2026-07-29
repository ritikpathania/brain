//! SQLite implementation of CheckpointStore, SnapshotStore, and EventStore.

use crate::connection::init_pool;
use crate::connection::SqliteConnectionManager;
use crate::errors::StorageError;
use crate::traits::{CheckpointStore, SnapshotStore};
use brain_events::{EventStore, ReflectionEventEnvelope};
use r2d2::Pool;

/// SQLite persistent implementation of `CheckpointStore`.
pub struct SqliteCheckpointStore {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCheckpointStore {
    /// Creates a new `SqliteCheckpointStore` with an existing pool.
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        self::init_schema(&pool).expect("Failed to initialize checkpoint schema");
        Self { pool }
    }

    /// Initializes in-memory or file database pool.
    pub fn in_memory() -> Self {
        let pool = init_pool(":memory:", 5, true).expect("Failed to init in-memory pool");
        Self::new(pool)
    }
}

fn init_schema(pool: &Pool<SqliteConnectionManager>) -> Result<(), StorageError> {
    let conn = pool
        .get()
        .map_err(|e| StorageError::Internal(e.to_string()))?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS plan_checkpoints (
            plan_id TEXT PRIMARY KEY,
            checkpoint_json TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS state_snapshots (
            snapshot_id TEXT PRIMARY KEY,
            data BLOB NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS event_envelopes (
            event_id TEXT PRIMARY KEY,
            plan_id TEXT NOT NULL,
            task_id TEXT,
            correlation_id TEXT NOT NULL,
            timestamp_ms INTEGER NOT NULL,
            envelope_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_env_plan ON event_envelopes(plan_id);
        CREATE INDEX IF NOT EXISTS idx_env_ts ON event_envelopes(timestamp_ms);
        "#,
    )
    .map_err(|e| StorageError::Internal(e.to_string()))?;
    Ok(())
}

impl CheckpointStore for SqliteCheckpointStore {
    fn save_checkpoint(&self, plan_id: &str, checkpoint_json: &str) -> Result<(), StorageError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        conn.execute(
            "INSERT INTO plan_checkpoints (plan_id, checkpoint_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(plan_id) DO UPDATE SET checkpoint_json = ?2, updated_at = ?3",
            rusqlite::params![plan_id, checkpoint_json, now],
        )
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(())
    }

    fn load_checkpoint(&self, plan_id: &str) -> Result<Option<String>, StorageError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT checkpoint_json FROM plan_checkpoints WHERE plan_id = ?1")
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let mut rows = stmt
            .query(rusqlite::params![plan_id])
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| StorageError::Internal(e.to_string()))?
        {
            let json: String = row
                .get(0)
                .map_err(|e| StorageError::Internal(e.to_string()))?;
            Ok(Some(json))
        } else {
            Ok(None)
        }
    }
}

/// SQLite persistent implementation of `SnapshotStore`.
pub struct SqliteSnapshotStore {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSnapshotStore {
    /// Creates a new `SqliteSnapshotStore`.
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        self::init_schema(&pool).expect("Failed to initialize snapshot schema");
        Self { pool }
    }

    /// Initializes in-memory snapshot store.
    pub fn in_memory() -> Self {
        let pool = init_pool(":memory:", 5, true).expect("Failed to init in-memory pool");
        Self::new(pool)
    }
}

impl SnapshotStore for SqliteSnapshotStore {
    fn save_snapshot(&self, snapshot_id: &str, data: &[u8]) -> Result<(), StorageError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        conn.execute(
            "INSERT INTO state_snapshots (snapshot_id, data, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(snapshot_id) DO UPDATE SET data = ?2, updated_at = ?3",
            rusqlite::params![snapshot_id, data, now],
        )
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(())
    }

    fn load_snapshot(&self, snapshot_id: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT data FROM state_snapshots WHERE snapshot_id = ?1")
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let mut rows = stmt
            .query(rusqlite::params![snapshot_id])
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| StorageError::Internal(e.to_string()))?
        {
            let data: Vec<u8> = row
                .get(0)
                .map_err(|e| StorageError::Internal(e.to_string()))?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }
}

/// SQLite persistent implementation of `EventStore`.
pub struct SqliteEventStore {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteEventStore {
    /// Creates a new `SqliteEventStore`.
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        self::init_schema(&pool).expect("Failed to initialize event schema");
        Self { pool }
    }

    /// Initializes in-memory event store.
    pub fn in_memory() -> Self {
        let pool = init_pool(":memory:", 5, true).expect("Failed to init in-memory pool");
        Self::new(pool)
    }
}

impl EventStore for SqliteEventStore {
    fn append(&self, envelope: ReflectionEventEnvelope) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let json = serde_json::to_string(&envelope).map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO event_envelopes (event_id, plan_id, task_id, correlation_id, timestamp_ms, envelope_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                envelope.event_id.to_string(),
                envelope.plan_id,
                envelope.task_id,
                envelope.correlation_id.to_string(),
                envelope.timestamp_ms as i64,
                json
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn query(&self, plan_id: &str) -> Vec<ReflectionEventEnvelope> {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut stmt = match conn.prepare("SELECT envelope_json FROM event_envelopes WHERE plan_id = ?1 ORDER BY timestamp_ms ASC") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = match stmt.query_map(rusqlite::params![plan_id], |row| {
            let json: String = row.get(0)?;
            Ok(json)
        }) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        let mut result = Vec::new();
        for json in rows.flatten() {
            if let Ok(env) = serde_json::from_str::<ReflectionEventEnvelope>(&json) {
                result.push(env);
            }
        }
        result
    }

    fn stream(&self) -> Vec<ReflectionEventEnvelope> {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut stmt = match conn
            .prepare("SELECT envelope_json FROM event_envelopes ORDER BY timestamp_ms ASC")
        {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = match stmt.query_map([], |row| {
            let json: String = row.get(0)?;
            Ok(json)
        }) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        let mut result = Vec::new();
        for json in rows.flatten() {
            if let Ok(env) = serde_json::from_str::<ReflectionEventEnvelope>(&json) {
                result.push(env);
            }
        }
        result
    }

    fn compact(&self, before_timestamp_ms: u64) -> usize {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return 0,
        };

        conn.execute(
            "DELETE FROM event_envelopes WHERE timestamp_ms < ?1",
            rusqlite::params![before_timestamp_ms as i64],
        )
        .unwrap_or(0)
    }
}
