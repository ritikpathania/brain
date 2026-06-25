use crate::storage::duckdb::AnalyticsDatabase;
use serde::Serialize;

#[derive(Serialize)]
pub struct AnalyticsSummary {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub total_queries: usize,
    pub cache_hit_rate: f64,
    pub avg_query_latency_us: f64,
    pub total_ingests: usize,
}

#[derive(Serialize)]
pub struct TypeDistribution {
    pub node_type: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct CentralNode {
    pub node_id: String,
    pub degree: usize,
}

#[derive(Serialize)]
pub struct GraphInsights {
    pub type_distribution: Vec<TypeDistribution>,
    pub degree_centrality: Vec<CentralNode>,
}

#[derive(Serialize)]
pub struct SimilarityPair {
    pub node_a: String,
    pub node_b: String,
    pub shared_neighbors: usize,
}

#[derive(Serialize)]
pub struct SlowQueryRecord {
    pub query_text: String,
    pub hit_type: String,
    pub execution_time_us: u64,
    pub timestamp: String,
}

#[derive(Serialize)]
pub struct LatencyMetrics {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub top_slow_queries: Vec<SlowQueryRecord>,
}

impl AnalyticsDatabase {
    /// Query summary analytics
    pub fn get_summary(&self) -> Result<AnalyticsSummary, duckdb::Error> {
        let conn_guard = self.conn.lock().unwrap();

        let total_nodes: usize =
            conn_guard.query_row("SELECT COUNT(*) FROM analytics_nodes", [], |row| row.get(0))?;

        let total_edges: usize =
            conn_guard.query_row("SELECT COUNT(*) FROM analytics_edges", [], |row| row.get(0))?;

        let total_ingests: usize =
            conn_guard.query_row("SELECT COUNT(*) FROM ingest_logs", [], |row| row.get(0))?;

        let mut stmt = conn_guard.prepare(
            "SELECT 
                 COUNT(*) as total,
                 COALESCE(SUM(CASE WHEN hit_type = 'STM' THEN 1 ELSE 0 END) * 1.0 / NULLIF(COUNT(*), 0), 0.0) as hit_rate,
                 COALESCE(AVG(execution_time_us), 0.0) as avg_lat
             FROM query_logs"
        )?;

        let mut rows = stmt.query([])?;
        let (total_queries, cache_hit_rate, avg_query_latency_us) =
            if let Some(row) = rows.next()? {
                let t: i64 = row.get(0)?;
                let h: f64 = row.get(1)?;
                let a: f64 = row.get(2)?;
                (t as usize, h, a)
            } else {
                (0, 0.0, 0.0)
            };

        Ok(AnalyticsSummary {
            total_nodes,
            total_edges,
            total_queries,
            cache_hit_rate,
            avg_query_latency_us,
            total_ingests,
        })
    }

    /// Query graph insights
    pub fn get_insights(&self) -> Result<GraphInsights, duckdb::Error> {
        let conn_guard = self.conn.lock().unwrap();

        // 1. Node type distribution
        let mut type_stmt = conn_guard.prepare(
            "SELECT type, COUNT(*) as count FROM analytics_nodes GROUP BY type ORDER BY count DESC",
        )?;
        let mut type_rows = type_stmt.query([])?;
        let mut type_distribution = Vec::new();
        while let Some(row) = type_rows.next()? {
            type_distribution.push(TypeDistribution {
                node_type: row.get(0)?,
                count: row.get::<_, i64>(1)? as usize,
            });
        }

        // 2. Degree centrality
        let mut deg_stmt = conn_guard.prepare(
            "SELECT node, COUNT(*) as degree FROM (
                 SELECT source as node FROM analytics_edges
                 UNION ALL
                 SELECT target as node FROM analytics_edges
             ) GROUP BY node ORDER BY degree DESC LIMIT 5",
        )?;
        let mut deg_rows = deg_stmt.query([])?;
        let mut degree_centrality = Vec::new();
        while let Some(row) = deg_rows.next()? {
            degree_centrality.push(CentralNode {
                node_id: row.get(0)?,
                degree: row.get::<_, i64>(1)? as usize,
            });
        }

        Ok(GraphInsights {
            type_distribution,
            degree_centrality,
        })
    }

    /// Query node similarities based on shared outbound connections
    pub fn get_similarity(&self) -> Result<Vec<SimilarityPair>, duckdb::Error> {
        let conn_guard = self.conn.lock().unwrap();
        let mut stmt = conn_guard.prepare(
            "SELECT 
                 e1.source as node_a, 
                 e2.source as node_b, 
                 COUNT(*) as shared_neighbors
             FROM analytics_edges e1
             JOIN analytics_edges e2 ON e1.target = e2.target AND e1.relation = e2.relation
             WHERE e1.source < e2.source
             GROUP BY node_a, node_b
             ORDER BY shared_neighbors DESC
             LIMIT 5",
        )?;

        let mut rows = stmt.query([])?;
        let mut similarity_report = Vec::new();
        while let Some(row) = rows.next()? {
            similarity_report.push(SimilarityPair {
                node_a: row.get(0)?,
                node_b: row.get(1)?,
                shared_neighbors: row.get::<_, i64>(2)? as usize,
            });
        }
        Ok(similarity_report)
    }

    /// Query slow queries and latency benchmarking percentiles
    pub fn get_latency_benchmarks(&self) -> Result<LatencyMetrics, duckdb::Error> {
        let conn_guard = self.conn.lock().unwrap();

        let mut pct_stmt = conn_guard.prepare(
            "SELECT 
                 COALESCE(quantile_cont(execution_time_us, 0.50), 0.0) as p50,
                 COALESCE(quantile_cont(execution_time_us, 0.95), 0.0) as p95,
                 COALESCE(quantile_cont(execution_time_us, 0.99), 0.0) as p99
             FROM query_logs",
        )?;
        let mut pct_rows = pct_stmt.query([])?;
        let (p50, p95, p99) = if let Some(row) = pct_rows.next()? {
            (
                row.get::<_, f64>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, f64>(2)?,
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        let mut slow_stmt = conn_guard.prepare(
            "SELECT query_text, hit_type, execution_time_us, strftime(timestamp, '%Y-%m-%d %H:%M:%S')
             FROM query_logs 
             ORDER BY execution_time_us DESC 
             LIMIT 5"
        )?;
        let mut slow_rows = slow_stmt.query([])?;
        let mut top_slow_queries = Vec::new();
        while let Some(row) = slow_rows.next()? {
            top_slow_queries.push(SlowQueryRecord {
                query_text: row.get(0)?,
                hit_type: row.get(1)?,
                execution_time_us: row.get::<_, i64>(2)? as u64,
                timestamp: row.get(3)?,
            });
        }

        Ok(LatencyMetrics {
            p50,
            p95,
            p99,
            top_slow_queries,
        })
    }
}
