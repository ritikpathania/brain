use rusqlite::Connection;

pub fn initialize_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    // Initialize schema tables
    conn.execute(
        "CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            label TEXT NOT NULL,
            type TEXT NOT NULL,
            properties TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS edges (
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            relation TEXT NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (source, target, relation),
            FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS node_embeddings (
            node_id TEXT PRIMARY KEY,
            embedding BLOB NOT NULL,
            centroid_id INTEGER,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );",
        [],
    )?;

    // Run migration: Add weight column to edges if missing
    let _ = conn.execute("ALTER TABLE edges ADD COLUMN weight REAL DEFAULT 1.0;", []);

    // Run migration: Add centroid_id column to node_embeddings if missing
    let _ = conn.execute(
        "ALTER TABLE node_embeddings ADD COLUMN centroid_id INTEGER;",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_node_embeddings_centroid ON node_embeddings(centroid_id);",
        [],
    );

    // Create query indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_nodes_type ON nodes(type);",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);",
        [],
    )?;

    Ok(())
}
