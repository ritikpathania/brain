use brain_core::BrainError;
use r2d2::ManageConnection;
use rusqlite::Connection;

/// Customizer for configuring SQLite connection properties on acquisition.
#[derive(Debug)]
pub struct SqliteConnectionCustomizer {
    /// Toggle WAL (Write-Ahead Logging) database mode.
    pub enable_wal: bool,
}

impl r2d2::CustomizeConnection<Connection, rusqlite::Error> for SqliteConnectionCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
        if self.enable_wal {
            conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        }
        if std::env::var("BRAIN_SQLITE_SYNCHRONOUS_NORMAL").is_ok() {
            conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
        }
        if std::env::var("BRAIN_SQLITE_TEMP_STORE_MEMORY").is_ok() {
            conn.execute_batch("PRAGMA temp_store = MEMORY;")?;
        }
        Ok(())
    }
}

/// Simple thread-safe connection manager for rusqlite.
#[derive(Debug)]
pub struct SqliteConnectionManager {
    path: std::path::PathBuf,
}

impl SqliteConnectionManager {
    /// Creates a new connection manager referencing a database path.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl ManageConnection for SqliteConnectionManager {
    type Connection = Connection;
    type Error = rusqlite::Error;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        if self.path.to_str() == Some(":memory:") {
            Connection::open_in_memory()
        } else {
            Connection::open(&self.path)
        }
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.execute_batch("")
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}

/// Initializes an r2d2 connection pool for SQLite.
pub fn init_pool(
    path: &str,
    pool_size: u32,
    enable_wal: bool,
) -> Result<r2d2::Pool<SqliteConnectionManager>, BrainError> {
    let manager = SqliteConnectionManager::new(path);

    let timeout_ms = std::env::var("BRAIN_DATABASE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(15000);
    let timeout = std::time::Duration::from_millis(timeout_ms);

    r2d2::Pool::builder()
        .max_size(pool_size)
        .connection_timeout(timeout)
        .connection_customizer(Box::new(SqliteConnectionCustomizer { enable_wal }))
        .build(manager)
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to create connection pool: {}", e),
            source: Some(Box::new(e)),
        })
}
