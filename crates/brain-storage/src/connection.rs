use brain_core::BrainError;
use rusqlite::Connection;

#[derive(Debug)]
pub struct SqliteConnectionCustomizer {
    pub enable_wal: bool,
}

impl r2d2::CustomizeConnection<Connection, rusqlite::Error> for SqliteConnectionCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        if self.enable_wal {
            conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ConnectionManager {
    path: std::path::PathBuf,
}

impl ConnectionManager {
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl r2d2::ManageConnection for ConnectionManager {
    type Connection = ::rusqlite::Connection;
    type Error = ::rusqlite::Error;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        ::rusqlite::Connection::open(&self.path)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.execute_batch("")
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}

pub mod rusqlite {
    pub use ::rusqlite::Connection;
    pub use ::rusqlite::Error;
    pub use super::ConnectionManager;
}

pub fn init_pool(
    path: &str,
    pool_size: u32,
    enable_wal: bool,
) -> Result<r2d2::Pool<rusqlite::ConnectionManager>, BrainError> {
    let manager = rusqlite::ConnectionManager::new(path);
    r2d2::Pool::builder()
        .max_size(pool_size)
        .connection_customizer(Box::new(SqliteConnectionCustomizer { enable_wal }))
        .build(manager)
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to create connection pool: {}", e),
            source: Some(Box::new(e)),
        })
}
