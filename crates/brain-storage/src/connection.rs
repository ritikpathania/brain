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
    fn on_acquire(&self, _conn: &mut Connection) -> Result<(), rusqlite::Error> {
        Ok(())
    }
}

/// # SQLite WAL & Synchronization Durability Policy
///
/// SQLite connections configured via `SqliteConnectionManager` operate under
/// **Write-Ahead Logging (WAL)** mode with `PRAGMA synchronous = NORMAL`.
///
/// ## Durability Invariant & Rationale
///
/// 1. **Application & OS Crash Safety**:
///    Under WAL mode with `synchronous = NORMAL`, SQLite syncs the WAL file during
///    WAL checkpoints rather than issuing a full disk `fsync` on every individual transaction.
///    In the event of an application crash (`SIGSEGV`, panic, `SIGKILL`) or OS crash,
///    the SQLite database and WAL log remain completely consistent and corruption-free.
///
/// 2. **Power-Loss Tradeoff**:
///    The only scenario where recently committed transactions could be lost is sudden
///    uncontrolled hardware power-loss between checkpoints. For a local AI companion and
///    session memory daemon, this durability guarantee satisfies production requirements
///    while eliminating 5-20ms disk fsync stalls on high-frequency streaming turns.
///
/// 3. **Connection Lifecycle Invariant**:
///    PRAGMA configuration is executed once upon connection creation in `ManageConnection::connect`,
///    ensuring connection acquisition from the pool is zero-cost.
#[derive(Debug)]
pub struct SqliteConnectionManager {
    path: std::path::PathBuf,
    enable_wal: bool,
}

impl SqliteConnectionManager {
    /// Creates a new connection manager referencing a database path.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            enable_wal: true,
        }
    }

    /// Creates a new connection manager with explicit WAL configuration.
    pub fn with_wal<P: AsRef<std::path::Path>>(path: P, enable_wal: bool) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            enable_wal,
        }
    }
}

impl ManageConnection for SqliteConnectionManager {
    type Connection = Connection;
    type Error = rusqlite::Error;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let conn = if self.path.to_str() == Some(":memory:") {
            Connection::open_in_memory()?
        } else {
            Connection::open(&self.path)?
        };

        conn.execute_batch(
            "PRAGMA foreign_keys = ON;\
             PRAGMA busy_timeout = 5000;\
             PRAGMA synchronous = NORMAL;\
             PRAGMA temp_store = MEMORY;",
        )?;

        if self.enable_wal {
            conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        }

        Ok(conn)
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
    let manager = SqliteConnectionManager::with_wal(path, enable_wal);

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
