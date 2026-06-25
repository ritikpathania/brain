use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tracing::{error, info};

use crate::storage::duckdb::AnalyticsDatabase;
use crate::DaemonMetrics;

pub async fn start_health_server(
    metrics: Arc<DaemonMetrics>,
    analytics_db: Arc<AnalyticsDatabase>,
) {
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:8080").await {
        Ok(l) => l,
        Err(e) => {
            error!(
                component = "observability",
                "Failed to bind health HTTP listener: {}", e
            );
            return;
        }
    };
    info!(
        component = "observability",
        port = 8080,
        "Health, Readiness & Metrics HTTP server running on http://127.0.0.1:8080"
    );

    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            let metrics_ref = Arc::clone(&metrics);
            let analytics_ref = Arc::clone(&analytics_db);
            tokio::spawn(async move {
                let mut buffer = [0; 1024];
                if stream.readable().await.is_ok() {
                    let _ = stream.try_read(&mut buffer);
                    let request = String::from_utf8_lossy(&buffer);

                    let (status, content_type, body) = if request.contains("GET /health ") {
                        (
                            "200 OK",
                            "application/json",
                            r#"{"status":"ok"}"#.to_string(),
                        )
                    } else if request.contains("GET /ready ") {
                        (
                            "200 OK",
                            "application/json",
                            r#"{"status":"ready"}"#.to_string(),
                        )
                    } else if request.contains("GET /metrics/json ") {
                        let hits = metrics_ref.cache_hits.load(Ordering::Relaxed);
                        let misses = metrics_ref.cache_misses.load(Ordering::Relaxed);
                        let total_q = metrics_ref.total_queries.load(Ordering::Relaxed);
                        let total_i = metrics_ref.total_ingests.load(Ordering::Relaxed);
                        let active = metrics_ref.active_workers.load(Ordering::Relaxed);
                        let queue = metrics_ref.stm_queue_depth.load(Ordering::Relaxed);

                        let q_lat_us = if total_q > 0 {
                            metrics_ref.sum_query_latency_us.load(Ordering::Relaxed) as f64
                                / total_q as f64
                        } else {
                            0.0
                        };
                        let i_lat_us = if total_i > 0 {
                            metrics_ref.sum_ingest_latency_us.load(Ordering::Relaxed) as f64
                                / total_i as f64
                        } else {
                            0.0
                        };
                        let ext_lat_us = if total_i > 0 {
                            metrics_ref
                                .sum_extraction_latency_us
                                .load(Ordering::Relaxed) as f64
                                / total_i as f64
                        } else {
                            0.0
                        };
                        let sql_lat_us = if total_i > 0 {
                            metrics_ref.sum_sqlite_latency_us.load(Ordering::Relaxed) as f64
                                / total_i as f64
                        } else {
                            0.0
                        };
                        let ipc_lat_us = if total_q + total_i > 0 {
                            metrics_ref.sum_ipc_latency_us.load(Ordering::Relaxed) as f64
                                / (total_q + total_i) as f64
                        } else {
                            0.0
                        };

                        let hit_rate = if hits + misses > 0 {
                            hits as f64 / (hits + misses) as f64
                        } else {
                            0.0
                        };

                        let response_body = format!(
                            r#"{{"cache_hit_rate":{},"cache_hits":{},"cache_misses":{},"total_queries":{},"total_ingests":{},"active_workers":{},"queue_depth":{},"avg_query_latency_us":{},"avg_ingest_latency_us":{},"avg_extraction_latency_us":{},"avg_sqlite_latency_us":{},"avg_ipc_latency_us":{}}}"#,
                            hit_rate,
                            hits,
                            misses,
                            total_q,
                            total_i,
                            active,
                            queue,
                            q_lat_us,
                            i_lat_us,
                            ext_lat_us,
                            sql_lat_us,
                            ipc_lat_us
                        );
                        ("200 OK", "application/json", response_body)
                    } else if request.contains("GET /metrics ") {
                        let hits = metrics_ref.cache_hits.load(Ordering::Relaxed);
                        let misses = metrics_ref.cache_misses.load(Ordering::Relaxed);
                        let total_q = metrics_ref.total_queries.load(Ordering::Relaxed);
                        let total_i = metrics_ref.total_ingests.load(Ordering::Relaxed);
                        let active = metrics_ref.active_workers.load(Ordering::Relaxed);
                        let queue = metrics_ref.stm_queue_depth.load(Ordering::Relaxed);

                        let q_lat_us = if total_q > 0 {
                            metrics_ref.sum_query_latency_us.load(Ordering::Relaxed) as f64
                                / total_q as f64
                        } else {
                            0.0
                        };
                        let i_lat_us = if total_i > 0 {
                            metrics_ref.sum_ingest_latency_us.load(Ordering::Relaxed) as f64
                                / total_i as f64
                        } else {
                            0.0
                        };
                        let ext_lat_us = if total_i > 0 {
                            metrics_ref
                                .sum_extraction_latency_us
                                .load(Ordering::Relaxed) as f64
                                / total_i as f64
                        } else {
                            0.0
                        };
                        let sql_lat_us = if total_i > 0 {
                            metrics_ref.sum_sqlite_latency_us.load(Ordering::Relaxed) as f64
                                / total_i as f64
                        } else {
                            0.0
                        };
                        let ipc_lat_us = if total_q + total_i > 0 {
                            metrics_ref.sum_ipc_latency_us.load(Ordering::Relaxed) as f64
                                / (total_q + total_i) as f64
                        } else {
                            0.0
                        };

                        let hit_rate = if hits + misses > 0 {
                            hits as f64 / (hits + misses) as f64
                        } else {
                            0.0
                        };

                        let q_lat_sec = q_lat_us / 1_000_000.0;
                        let i_lat_sec = i_lat_us / 1_000_000.0;
                        let ext_lat_sec = ext_lat_us / 1_000_000.0;
                        let sql_lat_sec = sql_lat_us / 1_000_000.0;
                        let ipc_lat_sec = ipc_lat_us / 1_000_000.0;

                        let prometheus_body = format!(
                            r#"# HELP brain_cache_hit_rate Rate of queries served from volatile short-term memory
# TYPE brain_cache_hit_rate gauge
brain_cache_hit_rate {}
# HELP brain_cache_hits_total Total count of cache hits
# TYPE brain_cache_hits_total counter
brain_cache_hits_total {}
# HELP brain_cache_misses_total Total count of cache misses
# TYPE brain_cache_misses_total counter
brain_cache_misses_total {}
# HELP brain_queries_total Total client queries processed
# TYPE brain_queries_total counter
brain_queries_total {}
# HELP brain_ingests_total Total client ingests processed
# TYPE brain_ingests_total counter
brain_ingests_total {}
# HELP brain_active_workers Count of active processing workers
# TYPE brain_active_workers gauge
brain_active_workers {}
# HELP brain_queue_depth Size of the active transient memory window
# TYPE brain_queue_depth gauge
brain_queue_depth {}
# HELP brain_avg_query_latency_seconds Average query processing time in seconds
# TYPE brain_avg_query_latency_seconds gauge
brain_avg_query_latency_seconds {}
# HELP brain_avg_ingest_latency_seconds Average ingest processing time in seconds
# TYPE brain_avg_ingest_latency_seconds gauge
brain_avg_ingest_latency_seconds {}
# HELP brain_avg_extraction_latency_seconds Average extraction processing time in seconds
# TYPE brain_avg_extraction_latency_seconds gauge
brain_avg_extraction_latency_seconds {}
# HELP brain_avg_sqlite_latency_seconds Average SQLite write latency in seconds
# TYPE brain_avg_sqlite_latency_seconds gauge
brain_avg_sqlite_latency_seconds {}
# HELP brain_avg_ipc_latency_seconds Average IPC roundtrip latency in seconds
# TYPE brain_avg_ipc_latency_seconds gauge
brain_avg_ipc_latency_seconds {}
"#,
                            hit_rate,
                            hits,
                            misses,
                            total_q,
                            total_i,
                            active,
                            queue,
                            q_lat_sec,
                            i_lat_sec,
                            ext_lat_sec,
                            sql_lat_sec,
                            ipc_lat_sec
                        );
                        ("200 OK", "text/plain; version=0.0.4", prometheus_body)
                    } else if request.contains("GET /analytics/summary ") {
                        match analytics_ref.get_summary() {
                            Ok(sum) => (
                                "200 OK",
                                "application/json",
                                serde_json::to_string(&sum).unwrap_or_default(),
                            ),
                            Err(e) => (
                                "500 INTERNAL SERVER ERROR",
                                "application/json",
                                format!(r#"{{"error":"{}"}}"#, e),
                            ),
                        }
                    } else if request.contains("GET /analytics/insights ") {
                        match analytics_ref.get_insights() {
                            Ok(ins) => (
                                "200 OK",
                                "application/json",
                                serde_json::to_string(&ins).unwrap_or_default(),
                            ),
                            Err(e) => (
                                "500 INTERNAL SERVER ERROR",
                                "application/json",
                                format!(r#"{{"error":"{}"}}"#, e),
                            ),
                        }
                    } else if request.contains("GET /analytics/similarity ") {
                        match analytics_ref.get_similarity() {
                            Ok(sim) => (
                                "200 OK",
                                "application/json",
                                serde_json::to_string(&sim).unwrap_or_default(),
                            ),
                            Err(e) => (
                                "500 INTERNAL SERVER ERROR",
                                "application/json",
                                format!(r#"{{"error":"{}"}}"#, e),
                            ),
                        }
                    } else if request.contains("GET /analytics/slow-queries ") {
                        match analytics_ref.get_latency_benchmarks() {
                            Ok(lat) => (
                                "200 OK",
                                "application/json",
                                serde_json::to_string(&lat).unwrap_or_default(),
                            ),
                            Err(e) => (
                                "500 INTERNAL SERVER ERROR",
                                "application/json",
                                format!(r#"{{"error":"{}"}}"#, e),
                            ),
                        }
                    } else {
                        (
                            "404 NOT FOUND",
                            "application/json",
                            r#"{"error":"not found"}"#.to_string(),
                        )
                    };

                    let response = format!(
                        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status, content_type, body.len(), body
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                }
            });
        }
    }
}
