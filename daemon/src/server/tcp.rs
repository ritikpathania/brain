use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tracing::{error, info};

use crate::DaemonMetrics;
use brain_services::BrainRuntime;

pub async fn start_health_server(metrics: Arc<DaemonMetrics>, brain_runtime: Arc<BrainRuntime>) {
    let port = std::env::var("BRAIN_HEALTH_PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("127.0.0.1:{}", port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            error!(
                component = "observability",
                "Failed to bind health HTTP listener on {}: {}", addr, e
            );
            return;
        }
    };
    info!(
        component = "observability",
        port = %port,
        "Health, Readiness & Metrics HTTP server running on http://{}", addr
    );

    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            let metrics_ref = Arc::clone(&metrics);
            let runtime_ref = Arc::clone(&brain_runtime);
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
                        let rt_metrics = runtime_ref.metrics();
                        let total_q = metrics_ref.total_queries.load(Ordering::Relaxed);
                        let total_i = metrics_ref.total_ingests.load(Ordering::Relaxed);
                        let active = metrics_ref.active_workers.load(Ordering::Relaxed);

                        let rt_attempts = total_i; // Simple parity
                        let rt_successes = rt_metrics.canonicalization_successes;
                        let rt_failures = rt_metrics.canonicalization_failures;
                        let rt_success_rate = if rt_attempts > 0 {
                            rt_successes as f64 / rt_attempts as f64
                        } else {
                            0.0
                        };

                        let rt_avg_lat_us = if rt_successes > 0 {
                            metrics_ref
                                .runtime_ingest_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64
                        } else {
                            0.0
                        };

                        let rt_canon_lat_us = if rt_successes > 0 {
                            metrics_ref
                                .runtime_canonicalization_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64
                        } else {
                            0.0
                        };

                        let rt_reflect_lat_us = if rt_successes > 0 {
                            metrics_ref
                                .runtime_reflection_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64
                        } else {
                            0.0
                        };

                        let rt_dispatch_lat_us = if rt_successes > 0 {
                            metrics_ref
                                .runtime_dispatch_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64
                        } else {
                            0.0
                        };

                        let (p50, p95, p99) =
                            if let Ok(reservoir) = metrics_ref.runtime_latency_reservoir.lock() {
                                reservoir.percentiles()
                            } else {
                                (0, 0, 0)
                            };

                        let response_body = format!(
                            r#"{{"cache_hit_rate":1.0,"cache_hits":0,"cache_misses":0,"total_queries":{},"total_ingests":{},"active_workers":{},"queue_depth":0,"avg_query_latency_us":0.0,"avg_ingest_latency_us":0.0,"avg_extraction_latency_us":0.0,"avg_sqlite_latency_us":0.0,"avg_ipc_latency_us":0.0,"runtime_ingest_attempts":{},"runtime_ingest_successes":{},"runtime_ingest_failures":{},"runtime_ingest_success_rate":{},"runtime_avg_ingest_latency_us":{},"legacy_avg_ingest_latency_us":0.0,"runtime_ingest_latency_ratio":1.0,"runtime_avg_canonicalization_us":{},"runtime_avg_reflection_us":{},"runtime_avg_dispatch_us":{},"runtime_p50_latency_us":{},"runtime_p95_latency_us":{},"runtime_p99_latency_us":{}}}"#,
                            total_q,
                            total_i,
                            active,
                            rt_attempts,
                            rt_successes,
                            rt_failures,
                            rt_success_rate,
                            rt_avg_lat_us,
                            rt_canon_lat_us,
                            rt_reflect_lat_us,
                            rt_dispatch_lat_us,
                            p50,
                            p95,
                            p99
                        );
                        ("200 OK", "application/json", response_body)
                    } else if request.contains("GET /metrics ") {
                        let rt_metrics = runtime_ref.metrics();
                        let total_q = metrics_ref.total_queries.load(Ordering::Relaxed);
                        let total_i = metrics_ref.total_ingests.load(Ordering::Relaxed);
                        let active = metrics_ref.active_workers.load(Ordering::Relaxed);

                        let rt_attempts = total_i;
                        let rt_successes = rt_metrics.canonicalization_successes;
                        let rt_failures = rt_metrics.canonicalization_failures;
                        let rt_success_rate = if rt_attempts > 0 {
                            rt_successes as f64 / rt_attempts as f64
                        } else {
                            0.0
                        };

                        let rt_avg_lat_us = if rt_successes > 0 {
                            metrics_ref
                                .runtime_ingest_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64
                        } else {
                            0.0
                        };

                        let rt_avg_lat_sec = rt_avg_lat_us / 1_000_000.0;
                        let rt_canon_lat_sec = if rt_successes > 0 {
                            (metrics_ref
                                .runtime_canonicalization_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64)
                                / 1_000_000.0
                        } else {
                            0.0
                        };
                        let rt_reflect_lat_sec = if rt_successes > 0 {
                            (metrics_ref
                                .runtime_reflection_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64)
                                / 1_000_000.0
                        } else {
                            0.0
                        };
                        let rt_dispatch_lat_sec = if rt_successes > 0 {
                            (metrics_ref
                                .runtime_dispatch_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64)
                                / 1_000_000.0
                        } else {
                            0.0
                        };

                        let (p50, p95, p99) =
                            if let Ok(reservoir) = metrics_ref.runtime_latency_reservoir.lock() {
                                reservoir.percentiles()
                            } else {
                                (0, 0, 0)
                            };
                        let p50_sec = p50 as f64 / 1_000_000.0;
                        let p95_sec = p95 as f64 / 1_000_000.0;
                        let p99_sec = p99 as f64 / 1_000_000.0;

                        let prometheus_body = format!(
                            r#"# HELP brain_cache_hit_rate Rate of queries served from volatile short-term memory
# TYPE brain_cache_hit_rate gauge
brain_cache_hit_rate 1.0
# HELP brain_cache_hits_total Total count of cache hits
# TYPE brain_cache_hits_total counter
brain_cache_hits_total 0
# HELP brain_cache_misses_total Total count of cache misses
# TYPE brain_cache_misses_total counter
brain_cache_misses_total 0
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
brain_queue_depth 0
# HELP brain_avg_query_latency_seconds Average query processing time in seconds
# TYPE brain_avg_query_latency_seconds gauge
brain_avg_query_latency_seconds 0.0
# HELP brain_avg_ingest_latency_seconds Average legacy ingest processing time in seconds
# TYPE brain_avg_ingest_latency_seconds gauge
brain_avg_ingest_latency_seconds 0.0
# HELP brain_avg_extraction_latency_seconds Average extraction processing time in seconds
# TYPE brain_avg_extraction_latency_seconds gauge
brain_avg_extraction_latency_seconds 0.0
# HELP brain_avg_sqlite_latency_seconds Average SQLite write latency in seconds
# TYPE brain_avg_sqlite_latency_seconds gauge
brain_avg_sqlite_latency_seconds 0.0
# HELP brain_avg_ipc_latency_seconds Average IPC roundtrip latency in seconds
# TYPE brain_avg_ipc_latency_seconds gauge
brain_avg_ipc_latency_seconds 0.0
# HELP brain_runtime_ingest_attempts_total BrainRuntime ingest attempts
# TYPE brain_runtime_ingest_attempts_total counter
brain_runtime_ingest_attempts_total {}
# HELP brain_runtime_ingest_successes_total BrainRuntime ingest successes
# TYPE brain_runtime_ingest_successes_total counter
brain_runtime_ingest_successes_total {}
# HELP brain_runtime_ingest_failures_total BrainRuntime ingest failures
# TYPE brain_runtime_ingest_failures_total counter
brain_runtime_ingest_failures_total {}
# HELP brain_runtime_ingest_success_rate Fraction of runtime ingests that succeeded
# TYPE brain_runtime_ingest_success_rate gauge
brain_runtime_ingest_success_rate {}
# HELP brain_runtime_avg_ingest_latency_seconds Average BrainRuntime ingest latency in seconds
# TYPE brain_runtime_avg_ingest_latency_seconds gauge
brain_runtime_avg_ingest_latency_seconds {}
# HELP brain_runtime_ingest_latency_ratio Runtime latency relative to legacy
# TYPE brain_runtime_ingest_latency_ratio gauge
brain_runtime_ingest_latency_ratio 1.0
# HELP brain_runtime_avg_canonicalization_seconds Average BrainRuntime canonicalization stage latency in seconds
# TYPE brain_runtime_avg_canonicalization_seconds gauge
brain_runtime_avg_canonicalization_seconds {}
# HELP brain_runtime_avg_reflection_seconds Average BrainRuntime reflection stage latency in seconds
# TYPE brain_runtime_avg_reflection_seconds gauge
brain_runtime_avg_reflection_seconds {}
# HELP brain_runtime_avg_dispatch_seconds Average BrainRuntime dispatch stage latency in seconds
# TYPE brain_runtime_avg_dispatch_seconds gauge
brain_runtime_avg_dispatch_seconds {}
# HELP brain_runtime_p50_latency_seconds P50 BrainRuntime ingest latency in seconds
# TYPE brain_runtime_p50_latency_seconds gauge
brain_runtime_p50_latency_seconds {}
# HELP brain_runtime_p95_latency_seconds P95 BrainRuntime ingest latency in seconds
# TYPE brain_runtime_p95_latency_seconds gauge
brain_runtime_p95_latency_seconds {}
# HELP brain_runtime_p99_latency_seconds P99 BrainRuntime ingest latency in seconds
# TYPE brain_runtime_p99_latency_seconds gauge
brain_runtime_p99_latency_seconds {}
"#,
                            total_q,
                            total_i,
                            active,
                            rt_attempts,
                            rt_successes,
                            rt_failures,
                            rt_success_rate,
                            rt_avg_lat_sec,
                            rt_canon_lat_sec,
                            rt_reflect_lat_sec,
                            rt_dispatch_lat_sec,
                            p50_sec,
                            p95_sec,
                            p99_sec
                        );
                        ("200 OK", "text/plain; version=0.0.4", prometheus_body)
                    } else if request.contains("GET /metrics/runtime ") {
                        let rt_metrics = runtime_ref.metrics();
                        let total_i = metrics_ref.total_ingests.load(Ordering::Relaxed);

                        let rt_attempts = total_i;
                        let rt_successes = rt_metrics.canonicalization_successes;
                        let rt_failures = rt_metrics.canonicalization_failures;
                        let rt_success_rate = if rt_attempts > 0 {
                            rt_successes as f64 / rt_attempts as f64
                        } else {
                            0.0
                        };
                        let rt_avg_lat_us = if rt_successes > 0 {
                            metrics_ref
                                .runtime_ingest_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64
                        } else {
                            0.0
                        };
                        let rt_canon_lat_us = if rt_successes > 0 {
                            metrics_ref
                                .runtime_canonicalization_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64
                        } else {
                            0.0
                        };
                        let rt_reflect_lat_us = if rt_successes > 0 {
                            metrics_ref
                                .runtime_reflection_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64
                        } else {
                            0.0
                        };
                        let rt_dispatch_lat_us = if rt_successes > 0 {
                            metrics_ref
                                .runtime_dispatch_latency_us
                                .load(Ordering::Relaxed) as f64
                                / rt_successes as f64
                        } else {
                            0.0
                        };

                        let (p50, p95, p99) =
                            if let Ok(reservoir) = metrics_ref.runtime_latency_reservoir.lock() {
                                reservoir.percentiles()
                            } else {
                                (0, 0, 0)
                            };

                        let response_body = format!(
                            r#"{{"status":"ok","ingests":{{"attempts":{},"successes":{},"failures":{},"success_rate":{}}},"latency":{{"avg_us":{},"p50_us":{},"p95_us":{},"p99_us":{}}},"stages":{{"canonicalization_avg_us":{},"reflection_avg_us":{},"dispatch_avg_us":{}}},"note":"Sampled from independent atomics. Use trends, not single scrapes."}}"#,
                            rt_attempts,
                            rt_successes,
                            rt_failures,
                            rt_success_rate,
                            rt_avg_lat_us,
                            p50,
                            p95,
                            p99,
                            rt_canon_lat_us,
                            rt_reflect_lat_us,
                            rt_dispatch_lat_us
                        );
                        ("200 OK", "application/json", response_body)
                    } else if request.contains("GET /analytics/summary ") {
                        (
                            "200 OK",
                            "application/json",
                            r#"{"nodes_count":0,"edges_count":0}"#.to_string(),
                        )
                    } else if request.contains("GET /analytics/insights ") {
                        (
                            "200 OK",
                            "application/json",
                            r#"{"density":0.0,"clustering_coefficient":0.0}"#.to_string(),
                        )
                    } else if request.contains("GET /analytics/similarity ") {
                        ("200 OK", "application/json", "[]".to_string())
                    } else if request.contains("GET /analytics/slow-queries ") {
                        (
                            "200 OK",
                            "application/json",
                            r#"{"p50_us":0.0,"p95_us":0.0,"p99_us":0.0}"#.to_string(),
                        )
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
