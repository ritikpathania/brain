use crate::store::SqliteStorage;
use std::fs;
use uuid::Uuid;

/// Reusable storage test fixture that manages database lifecycle.
pub struct TestStorage {
    storage: SqliteStorage,
    db_path: String,
}

impl TestStorage {
    /// Creates a new TestStorage fixture with a temporary, unique database file.
    pub fn new() -> Self {
        let mut temp_db = std::env::temp_dir();
        temp_db.push(format!("brain_test_{}.db", Uuid::new_v4()));
        let db_path = temp_db.to_str().unwrap().to_string();

        // Initialize with a connection pool size of 5 and WAL enabled
        let storage = SqliteStorage::new(&db_path, 5, true)
            .expect("Failed to initialize TestStorage SqliteStorage");

        Self { storage, db_path }
    }

    /// Exposes a reference to the underlying SqliteStorage backend.
    pub fn storage(&self) -> &SqliteStorage {
        &self.storage
    }

    /// Returns the underlying storage wrapped in an Arc.
    pub fn store(&self) -> std::sync::Arc<SqliteStorage> {
        std::sync::Arc::new(self.storage.clone())
    }

    /// Asserts that no database connections are currently leaked/active outside the pool.
    pub fn assert_clean(&self) {
        let state = self.storage.pool().state();
        assert_eq!(
            state.connections, state.idle_connections,
            "Connection leak detected! Total connections: {}, idle connections: {}",
            state.connections, state.idle_connections
        );
    }
}

impl Default for TestStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestStorage {
    fn drop(&mut self) {
        // Clean up main db file and transient WAL/SHM files
        let _ = fs::remove_file(&self.db_path);
        let _ = fs::remove_file(format!("{}-wal", self.db_path));
        let _ = fs::remove_file(format!("{}-shm", self.db_path));
    }
}
