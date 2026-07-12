use duckdb::params;

use crate::plugins::{Exporter, StorageBackend};
use crate::storage::duckdb::AnalyticsDatabase;

impl Exporter for AnalyticsDatabase {
    fn name(&self) -> &str {
        "duckdb"
    }

    fn export(&self, backend: &dyn StorageBackend) -> Result<(), String> {
        let mut duck_guard = self.conn.lock().unwrap();

        // 1. Get the last sync updated_at timestamp from DuckDB
        let mut last_sync: i64 = 0;
        let mut stmt = duck_guard
            .prepare("SELECT val_int FROM sync_metadata WHERE key = 'last_sync_updated_at'")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            last_sync = row.get(0).map_err(|e| e.to_string())?;
        }

        // 2. Fetch new/updated nodes and edges from StorageBackend
        let (nodes, edges, max_updated_at) = backend.get_updates_since(last_sync)?;

        if nodes.is_empty() && edges.is_empty() {
            return Ok(());
        }

        let tx = duck_guard.transaction().map_err(|e| e.to_string())?;
        {
            let mut insert_node_stmt = tx
                .prepare(
                    "INSERT INTO analytics_nodes (id, label, type, properties, updated_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                     label = excluded.label,
                     type = excluded.type,
                     properties = excluded.properties,
                     updated_at = excluded.updated_at",
                )
                .map_err(|e| e.to_string())?;

            for node in nodes {
                let props_str =
                    serde_json::to_string(&node.attributes).unwrap_or_else(|_| "{}".to_string());
                insert_node_stmt
                    .execute(params![
                        node.id,
                        node.label,
                        node.node_type,
                        props_str,
                        max_updated_at
                    ])
                    .map_err(|e| e.to_string())?;
            }
        }

        {
            let mut insert_edge_stmt = tx
                .prepare(
                    "INSERT INTO analytics_edges (source, target, relation, updated_at)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT(source, target, relation) DO UPDATE SET
                     updated_at = excluded.updated_at",
                )
                .map_err(|e| e.to_string())?;

            for edge in edges {
                insert_edge_stmt
                    .execute(params![
                        edge.source,
                        edge.target,
                        edge.relation,
                        max_updated_at
                    ])
                    .map_err(|e| e.to_string())?;
            }
        }

        // 4. Update the last sync watermark in DuckDB if we found any newer updates
        if max_updated_at > last_sync {
            tx.execute(
                "INSERT INTO sync_metadata (key, val_int) VALUES ('last_sync_updated_at', ?)
                 ON CONFLICT(key) DO UPDATE SET val_int = excluded.val_int",
                params![max_updated_at],
            )
            .map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
}
