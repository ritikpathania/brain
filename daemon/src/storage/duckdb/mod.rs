use duckdb::{params, Connection};
use std::sync::{Arc, Mutex};

pub mod analytics;
pub mod export;

pub use analytics::*;

#[derive(Debug, Clone)]
pub enum AnalyticsEvent {
    Query {
        correlation_id: u64,
        query_text: String,
        hit_type: String, // "STM", "LTM", "None"
        execution_time_us: u64,
    },
    Ingest {
        correlation_id: u64,
        node_id: String,
        content_length: u64,
    },
}

pub struct AnalyticsDatabase {
    pub conn: Arc<Mutex<Connection>>,
}

impl AnalyticsDatabase {
    pub fn new(db_path: &str) -> Result<Self, duckdb::Error> {
        let conn = Connection::open(db_path)?;

        // Initialize tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sync_metadata (
                key VARCHAR PRIMARY KEY,
                val_int BIGINT
            );
            CREATE TABLE IF NOT EXISTS analytics_nodes (
                id VARCHAR PRIMARY KEY,
                label VARCHAR,
                type VARCHAR,
                properties VARCHAR,
                updated_at BIGINT,
                exported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS analytics_edges (
                source VARCHAR,
                target VARCHAR,
                relation VARCHAR,
                updated_at BIGINT,
                exported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (source, target, relation)
            );
            CREATE TABLE IF NOT EXISTS query_logs (
                correlation_id BIGINT,
                query_text VARCHAR,
                hit_type VARCHAR,
                execution_time_us BIGINT,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS ingest_logs (
                correlation_id BIGINT,
                node_id VARCHAR,
                content_length BIGINT,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );",
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn new_in_memory() -> Result<Self, duckdb::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sync_metadata (
                key VARCHAR PRIMARY KEY,
                val_int BIGINT
            );
            CREATE TABLE IF NOT EXISTS analytics_nodes (
                id VARCHAR PRIMARY KEY,
                label VARCHAR,
                type VARCHAR,
                properties VARCHAR,
                updated_at BIGINT,
                exported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS analytics_edges (
                source VARCHAR,
                target VARCHAR,
                relation VARCHAR,
                updated_at BIGINT,
                exported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (source, target, relation)
            );
            CREATE TABLE IF NOT EXISTS query_logs (
                correlation_id BIGINT,
                query_text VARCHAR,
                hit_type VARCHAR,
                execution_time_us BIGINT,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS ingest_logs (
                correlation_id BIGINT,
                node_id VARCHAR,
                content_length BIGINT,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Record a client activity event into DuckDB
    pub fn record_event(&self, event: AnalyticsEvent) -> Result<(), duckdb::Error> {
        let conn_guard = self.conn.lock().unwrap();
        match event {
            AnalyticsEvent::Query {
                correlation_id,
                query_text,
                hit_type,
                execution_time_us,
            } => {
                conn_guard.execute(
                    "INSERT INTO query_logs (correlation_id, query_text, hit_type, execution_time_us) VALUES (?, ?, ?, ?)",
                    params![correlation_id as i64, query_text, hit_type, execution_time_us as i64],
                )?;
            }
            AnalyticsEvent::Ingest {
                correlation_id,
                node_id,
                content_length,
            } => {
                conn_guard.execute(
                    "INSERT INTO ingest_logs (correlation_id, node_id, content_length) VALUES (?, ?, ?)",
                    params![correlation_id as i64, node_id, content_length as i64],
                )?;
            }
        }
        Ok(())
    }

    /// Run the incremental synchronization from SQLite to DuckDB
    pub fn run_incremental_sync(
        &self,
        sqlite_conn: &rusqlite::Connection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut duck_guard = self.conn.lock().unwrap();

        // 1. Get the last sync updated_at timestamp from DuckDB
        let mut last_sync: i64 = 0;
        let mut stmt = duck_guard
            .prepare("SELECT val_int FROM sync_metadata WHERE key = 'last_sync_updated_at'")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            last_sync = row.get(0)?;
        }

        // 2. Fetch new/updated nodes from SQLite
        let mut sqlite_nodes_stmt = sqlite_conn.prepare(
            "SELECT id, label, type, properties, updated_at FROM nodes WHERE updated_at > ?1",
        )?;
        let mut node_rows = sqlite_nodes_stmt.query(rusqlite::params![last_sync])?;

        let mut new_max_updated_at = last_sync;

        let tx = duck_guard.transaction()?;
        {
            let mut insert_node_stmt = tx.prepare(
                "INSERT INTO analytics_nodes (id, label, type, properties, updated_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                     label = excluded.label,
                     type = excluded.type,
                     properties = excluded.properties,
                     updated_at = excluded.updated_at",
            )?;

            while let Some(row) = node_rows.next()? {
                let id: String = row.get(0)?;
                let label: String = row.get(1)?;
                let node_type: String = row.get(2)?;
                let properties: String = row.get(3)?;
                let updated_at: i64 = row.get(4)?;

                if updated_at > new_max_updated_at {
                    new_max_updated_at = updated_at;
                }

                insert_node_stmt.execute(params![id, label, node_type, properties, updated_at])?;
            }
        }

        // 3. Fetch new/updated edges from SQLite
        let mut sqlite_edges_stmt = sqlite_conn.prepare(
            "SELECT source, target, relation, updated_at FROM edges WHERE updated_at > ?1",
        )?;
        let mut edge_rows = sqlite_edges_stmt.query(rusqlite::params![last_sync])?;

        {
            let mut insert_edge_stmt = tx.prepare(
                "INSERT INTO analytics_edges (source, target, relation, updated_at)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT(source, target, relation) DO UPDATE SET
                     updated_at = excluded.updated_at",
            )?;

            while let Some(row) = edge_rows.next()? {
                let source: String = row.get(0)?;
                let target: String = row.get(1)?;
                let relation: String = row.get(2)?;
                let updated_at: i64 = row.get(3)?;

                if updated_at > new_max_updated_at {
                    new_max_updated_at = updated_at;
                }

                insert_edge_stmt.execute(params![source, target, relation, updated_at])?;
            }
        }

        // 4. Update the last sync watermark in DuckDB if we found any newer updates
        if new_max_updated_at > last_sync {
            tx.execute(
                "INSERT INTO sync_metadata (key, val_int) VALUES ('last_sync_updated_at', ?)
                 ON CONFLICT(key) DO UPDATE SET val_int = excluded.val_int",
                params![new_max_updated_at],
            )?;
        }

        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analytics_setup_and_events() {
        let db = AnalyticsDatabase::new_in_memory().unwrap();
        db.record_event(AnalyticsEvent::Query {
            correlation_id: 1,
            query_text: "test query".to_string(),
            hit_type: "STM".to_string(),
            execution_time_us: 150,
        })
        .unwrap();

        db.record_event(AnalyticsEvent::Ingest {
            correlation_id: 2,
            node_id: "test-node".to_string(),
            content_length: 200,
        })
        .unwrap();

        let summary = db.get_summary().unwrap();
        assert_eq!(summary.total_queries, 1);
        assert_eq!(summary.total_ingests, 1);
        assert_eq!(summary.cache_hit_rate, 1.0);
        assert_eq!(summary.avg_query_latency_us, 150.0);
    }

    #[test]
    fn test_duplicate_watermark_sync_no_panic() {
        let db = AnalyticsDatabase::new_in_memory().unwrap();
        let conn = db.conn.lock().unwrap();

        // 1. Initial insert of watermark
        conn.execute(
            "INSERT INTO sync_metadata (key, val_int) VALUES ('last_sync_updated_at', 100)
             ON CONFLICT(key) DO UPDATE SET val_int = excluded.val_int",
            params![],
        )
        .unwrap();

        // 2. Second insert of the same key (simulating duplicate watermark sync)
        conn.execute(
            "INSERT INTO sync_metadata (key, val_int) VALUES ('last_sync_updated_at', 200)
             ON CONFLICT(key) DO UPDATE SET val_int = excluded.val_int",
            params![],
        )
        .unwrap();

        // 3. Verify it was updated successfully to 200 without constraint violation
        let mut stmt = conn.prepare("SELECT val_int FROM sync_metadata WHERE key = 'last_sync_updated_at'").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let val: i64 = rows.next().unwrap().unwrap().get(0).unwrap();
        assert_eq!(val, 200);
    }
}
