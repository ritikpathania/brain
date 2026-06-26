use brain_core::BrainError;
use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    // Version 1 Schema Setup
    r#"
    CREATE TABLE IF NOT EXISTS nodes (
        id TEXT PRIMARY KEY,
        label TEXT NOT NULL,
        node_type TEXT NOT NULL,
        properties TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS edges (
        source TEXT NOT NULL,
        target TEXT NOT NULL,
        relation TEXT NOT NULL,
        weight REAL NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (source, target, relation),
        FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
        FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
    CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
    CREATE TABLE IF NOT EXISTS embeddings (
        node_id TEXT PRIMARY KEY,
        vector BLOB NOT NULL,
        dimension INTEGER NOT NULL,
        FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
    );
    CREATE TABLE IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY,
        history TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS config (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    "#,
];

/// Runs all pending database schema migrations in a transaction.
pub fn run_migrations(conn: &mut Connection) -> Result<(), BrainError> {
    let mut current_version: u32 = conn
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to query schema version: {}", e),
            source: Some(Box::new(e)),
        })?;

    for (idx, migration) in MIGRATIONS.iter().enumerate() {
        let version = (idx + 1) as u32;
        if version > current_version {
            let tx = conn.transaction().map_err(|e| BrainError::Storage {
                message: format!("Failed to start migration transaction: {}", e),
                source: Some(Box::new(e)),
            })?;

            tx.execute_batch(migration)
                .map_err(|e| BrainError::Storage {
                    message: format!("Migration version {} failed: {}", version, e),
                    source: Some(Box::new(e)),
                })?;

            tx.pragma_update(None, "user_version", version)
                .map_err(|e| BrainError::Storage {
                    message: format!("Failed to update user_version to {}: {}", version, e),
                    source: Some(Box::new(e)),
                })?;

            tx.commit().map_err(|e| BrainError::Storage {
                message: format!("Failed to commit migration version {}: {}", version, e),
                source: Some(Box::new(e)),
            })?;

            current_version = version;
        }
    }

    Ok(())
}
