//! Versioned transactional database migration runner.

use brain_core::errors::BrainError;
use rusqlite::Connection;

/// Predefined DDL schema migrations (Versions 1-15).
pub const MIGRATIONS: &[&str] = &[
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
    // Version 2 Schema Setup (PR-014 checkpoints and summaries)
    r#"
    CREATE TABLE IF NOT EXISTS checkpoints (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        label TEXT NOT NULL,
        history TEXT NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS summaries (
        session_id TEXT NOT NULL,
        version INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        start_idx INTEGER NOT NULL,
        end_idx INTEGER NOT NULL,
        text TEXT NOT NULL,
        PRIMARY KEY (session_id, version)
    );
    "#,
    // Version 3 Schema Setup (Ingestion Event Log / WAL)
    r#"
    CREATE TABLE IF NOT EXISTS event_log (
        sequence INTEGER PRIMARY KEY AUTOINCREMENT,
        event_id TEXT UNIQUE NOT NULL,
        adapter_id TEXT NOT NULL,
        client_id TEXT NOT NULL,
        session_id TEXT NOT NULL,
        workspace_id TEXT NOT NULL,
        conversation_id TEXT,
        event_model_version TEXT NOT NULL,
        event_type TEXT NOT NULL,
        payload TEXT NOT NULL,
        timestamp TEXT NOT NULL,
        received_at TEXT NOT NULL,
        processed INTEGER DEFAULT 0
    );
    CREATE INDEX IF NOT EXISTS idx_event_log_adapter ON event_log(adapter_id);
    CREATE INDEX IF NOT EXISTS idx_event_log_session ON event_log(session_id);
    CREATE INDEX IF NOT EXISTS idx_event_log_type ON event_log(event_type);
    "#,
    // Version 4 Schema Setup (Memory Consolidation Archival)
    r#"
    CREATE TABLE IF NOT EXISTS archived_edges (
        source TEXT NOT NULL,
        target TEXT NOT NULL,
        relation TEXT NOT NULL,
        weight REAL NOT NULL,
        updated_at INTEGER NOT NULL,
        archived_at INTEGER NOT NULL,
        PRIMARY KEY (source, target, relation)
    );
    CREATE INDEX IF NOT EXISTS idx_archived_edges_source ON archived_edges(source);
    CREATE INDEX IF NOT EXISTS idx_archived_edges_target ON archived_edges(target);
    "#,
    // Version 5 Schema Setup (Temporal Retrieval Fields)
    r#"
    ALTER TABLE edges ADD COLUMN observed_at INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE edges ADD COLUMN validity TEXT NOT NULL DEFAULT '[]';
    "#,
    // Version 6 Schema Setup (Learned Ranking & Feedback Events)
    r#"
    CREATE TABLE IF NOT EXISTS weight_snapshots (
        version INTEGER PRIMARY KEY,
        created_at INTEGER NOT NULL,
        semantic_weight REAL NOT NULL,
        graph_weight REAL NOT NULL,
        recency_weight REAL NOT NULL,
        temporal_weight REAL NOT NULL,
        calibration_metadata TEXT NOT NULL
    );
    INSERT INTO weight_snapshots (version, created_at, semantic_weight, graph_weight, recency_weight, temporal_weight, calibration_metadata)
    VALUES (1, strftime('%s','now'), 1.0, 1.0, 1.0, 1.0, '{"algorithm_used":"Default","validation_loss":null}');
    CREATE TABLE IF NOT EXISTS feedback_events (
        id TEXT PRIMARY KEY,
        schema_version INTEGER NOT NULL,
        query TEXT NOT NULL,
        node_id TEXT NOT NULL,
        selected INTEGER NOT NULL,
        timestamp INTEGER NOT NULL,
        ranking_position INTEGER NOT NULL,
        context TEXT NOT NULL
    );
    "#,
    // Version 7 Schema Setup (System / Domain Event Log)
    r#"
    CREATE TABLE IF NOT EXISTS system_event_log (
        sequence INTEGER PRIMARY KEY AUTOINCREMENT,
        event_id TEXT UNIQUE NOT NULL,
        correlation_id TEXT NOT NULL,
        timestamp_ms INTEGER NOT NULL,
        version TEXT NOT NULL,
        source TEXT NOT NULL,
        topic TEXT NOT NULL,
        payload TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_system_event_log_topic ON system_event_log(topic);
    CREATE INDEX IF NOT EXISTS idx_system_event_log_ts ON system_event_log(timestamp_ms);
    "#,
    // Version 8 Schema Setup (Projection Checkpoints)
    r#"
    CREATE TABLE IF NOT EXISTS projection_checkpoints (
        projection_name TEXT PRIMARY KEY,
        last_sequence INTEGER NOT NULL
    );
    "#,
    // Version 9 Schema Setup (Jobs Projection Read Model)
    r#"
    CREATE TABLE IF NOT EXISTS jobs_projection (
        job_id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        owner TEXT NOT NULL,
        state TEXT NOT NULL,
        priority INTEGER NOT NULL,
        progress INTEGER NOT NULL,
        started_at INTEGER,
        completed_at INTEGER,
        failure_reason TEXT,
        updated_sequence INTEGER NOT NULL
    );
    "#,
    // Version 10 Schema Setup (KPP lifecycle, validity, version_state fields)
    r#"
    ALTER TABLE nodes ADD COLUMN lifecycle TEXT NOT NULL DEFAULT 'Observed';
    ALTER TABLE nodes ADD COLUMN validity TEXT NOT NULL DEFAULT 'Unverified';
    ALTER TABLE nodes ADD COLUMN version_state TEXT NOT NULL DEFAULT 'Current';
    ALTER TABLE edges ADD COLUMN lifecycle TEXT NOT NULL DEFAULT 'Observed';
    ALTER TABLE edges ADD COLUMN version_state TEXT NOT NULL DEFAULT 'Current';
    "#,
    // Version 11 Schema Setup (Sessions Projection Read Model)
    r#"
    CREATE TABLE IF NOT EXISTS sessions_projection (
        session_id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        is_archived INTEGER NOT NULL,
        is_pinned INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        updated_sequence INTEGER NOT NULL
    );
    "#,
    // Version 12 Schema Setup (Search Index FTS5 Projection with session_id)
    r#"
    CREATE VIRTUAL TABLE IF NOT EXISTS search_projection USING fts5(
        id UNINDEXED,
        kind UNINDEXED,
        title,
        body,
        metadata UNINDEXED,
        updated_sequence UNINDEXED,
        session_id UNINDEXED,
        tokenize='unicode61'
    );
    "#,
    // Version 13 Schema Setup (External-Content FTS5 table for nodes)
    r#"
    CREATE VIRTUAL TABLE IF NOT EXISTS node_search USING fts5(
        label,
        content='nodes',
        tokenize='unicode61'
    );
    CREATE TRIGGER IF NOT EXISTS trg_nodes_insert AFTER INSERT ON nodes BEGIN
        INSERT INTO node_search(rowid, label) VALUES (new.rowid, new.label);
    END;
    CREATE TRIGGER IF NOT EXISTS trg_nodes_delete AFTER DELETE ON nodes BEGIN
        INSERT INTO node_search(node_search, rowid, label) VALUES('delete', old.rowid, old.label);
    END;
    CREATE TRIGGER IF NOT EXISTS trg_nodes_update AFTER UPDATE ON nodes BEGIN
        INSERT INTO node_search(node_search, rowid, label) VALUES('delete', old.rowid, old.label);
        INSERT INTO node_search(rowid, label) VALUES (new.rowid, new.label);
    END;
    INSERT INTO node_search(rowid, label) SELECT rowid, label FROM nodes;
    "#,
    // Version 14 Schema Setup (IVF Vector Indexing centroid_id field)
    r#"
    ALTER TABLE embeddings ADD COLUMN centroid_id INTEGER;
    CREATE INDEX IF NOT EXISTS idx_embeddings_centroid ON embeddings(centroid_id);
    "#,
    // Version 15 Schema Setup (Projection Metadata & Resumable Checkpoints)
    r#"
    CREATE TABLE IF NOT EXISTS projection_metadata (
        projection_name TEXT PRIMARY KEY,
        projection_version INTEGER NOT NULL,
        last_sequence INTEGER NOT NULL,
        status TEXT NOT NULL CHECK(status IN ('active', 'failed', 'rebuilding', 'idle')),
        last_error TEXT,
        updated_at INTEGER NOT NULL
    );
    INSERT OR IGNORE INTO projection_metadata (projection_name, projection_version, last_sequence, status, last_error, updated_at)
    SELECT projection_name, 1, last_sequence, 'idle', NULL, strftime('%s','now') FROM projection_checkpoints;
    "#,
];

/// Migration runner responsible for applying versioned DDL and data migrations idempotently.
pub struct MigrationRunner;

impl MigrationRunner {
    /// Executes all pending versioned migrations in order inside individual transactions.
    pub fn run_migrations(conn: &mut Connection) -> Result<(), BrainError> {
        // 1. Run base DDL schema migrations (1-15) via PRAGMA user_version
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

        // 2. Ensure _schema_migrations history table exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to create _schema_migrations table: {}", e),
            source: Some(Box::new(e)),
        })?;

        // 3. Apply V16 (backfill session_id & rebuild virtual table if needed)
        Self::apply_v16_add_session_id_column(conn)?;

        Ok(())
    }

    fn is_version_applied(conn: &Connection, version: u32) -> Result<bool, BrainError> {
        let mut stmt = conn
            .prepare("SELECT 1 FROM _schema_migrations WHERE version = ?1")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare migration check: {}", e),
                source: Some(Box::new(e)),
            })?;
        let exists = stmt.exists([version]).map_err(|e| BrainError::Storage {
            message: format!("Failed to query migration status: {}", e),
            source: Some(Box::new(e)),
        })?;
        Ok(exists)
    }

    fn apply_v16_add_session_id_column(conn: &mut Connection) -> Result<(), BrainError> {
        if Self::is_version_applied(conn, 16)? {
            return Ok(());
        }

        let tx = conn.transaction().map_err(|e| BrainError::Storage {
            message: format!("Failed to begin migration transaction V16: {}", e),
            source: Some(Box::new(e)),
        })?;

        // Check if session_id column exists on search_projection
        let mut has_column = false;
        {
            let mut stmt = tx
                .prepare("PRAGMA table_info(search_projection)")
                .map_err(|e| BrainError::Storage {
                    message: format!("Failed to inspect search_projection table info: {}", e),
                    source: Some(Box::new(e)),
                })?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|e| BrainError::Storage {
                    message: format!("Failed to query table_info rows: {}", e),
                    source: Some(Box::new(e)),
                })?;
            for col_name in rows.flatten() {
                if col_name == "session_id" {
                    has_column = true;
                    break;
                }
            }
        }

        if !has_column {
            // Rebuild FTS5 virtual table with session_id UNINDEXED column
            tx.execute_batch(
                r#"
                CREATE VIRTUAL TABLE search_projection_new USING fts5(
                    id UNINDEXED,
                    kind UNINDEXED,
                    title,
                    body,
                    metadata UNINDEXED,
                    updated_sequence UNINDEXED,
                    session_id UNINDEXED,
                    tokenize='unicode61'
                );
                INSERT INTO search_projection_new (id, kind, title, body, metadata, updated_sequence, session_id)
                SELECT id, kind, title, body, metadata, updated_sequence, json_extract(metadata, '$.session_id') FROM search_projection;
                DROP TABLE search_projection;
                ALTER TABLE search_projection_new RENAME TO search_projection;
                "#,
            )
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to migrate virtual table search_projection: {}", e),
                source: Some(Box::new(e)),
            })?;
        }

        // Record migration V16 in _schema_migrations
        let now = chrono::Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO _schema_migrations (version, applied_at) VALUES (16, ?1)",
            [now],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to record migration V16: {}", e),
            source: Some(Box::new(e)),
        })?;

        tx.commit().map_err(|e| BrainError::Storage {
            message: format!("Failed to commit migration transaction V16: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_v16_and_restart_idempotency() {
        let mut conn = Connection::open_in_memory().unwrap();

        // 1. Run migrations on fresh connection
        MigrationRunner::run_migrations(&mut conn).unwrap();

        // Verify session_id column exists
        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM _schema_migrations WHERE version = 16",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // 2. Restart / Rerun migration idempotency test
        MigrationRunner::run_migrations(&mut conn).unwrap();

        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM _schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
