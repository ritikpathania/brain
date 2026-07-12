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
    // Version 12 Schema Setup (Search Index FTS5 Projection)
    r#"
    CREATE VIRTUAL TABLE IF NOT EXISTS search_projection USING fts5(
        id UNINDEXED,
        kind UNINDEXED,
        title,
        body,
        metadata UNINDEXED,
        updated_sequence UNINDEXED,
        tokenize='unicode61'
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
