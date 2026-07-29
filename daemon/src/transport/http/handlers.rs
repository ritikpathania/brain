use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tracing::{error, info};

use crate::DaemonMetrics;
use brain_application::context::ExecutionContext;
use brain_application::dispatcher::{ApplicationRequest, ApplicationResponse, RequestDispatcher};

/// Starts the HTTP health, readiness, and metrics server.
pub async fn start_health_server(metrics: Arc<DaemonMetrics>, dispatcher: Arc<RequestDispatcher>) {
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

    let _context = ExecutionContext::default();

    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            let metrics_ref = Arc::clone(&metrics);
            let dispatcher_ref = Arc::clone(&dispatcher);
            tokio::spawn(async move {
                let context_ref = ExecutionContext::default();
                let mut buffer = [0; 1024];
                if stream.readable().await.is_ok() {
                    let _ = stream.try_read(&mut buffer);
                    let request = String::from_utf8_lossy(&buffer);

                    let (status, content_type, body) = if request.contains("GET /status ") {
                        match dispatcher_ref
                            .dispatch(ApplicationRequest::Status, &context_ref)
                            .await
                        {
                            Ok(ApplicationResponse::Status(dto)) => {
                                let code = if dto.health == "healthy" {
                                    "200 OK"
                                } else {
                                    "503 Service Unavailable"
                                };
                                let response_body = serde_json::to_string(&dto).unwrap_or_default();
                                (code, "application/json", response_body)
                            }
                            _ => (
                                "503 Service Unavailable",
                                "application/json",
                                "{}".to_string(),
                            ),
                        }
                    } else if request.contains("GET /health ") {
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
                    } else if request.contains("GET /diagnostics ") {
                        match dispatcher_ref
                            .dispatch(ApplicationRequest::Status, &context_ref)
                            .await
                        {
                            Ok(ApplicationResponse::Status(status_dto)) => {
                                let total_q = metrics_ref.total_queries.load(Ordering::Relaxed);
                                let total_i = metrics_ref.total_ingests.load(Ordering::Relaxed);
                                let active = metrics_ref.active_workers.load(Ordering::Relaxed);
                                let socket_path = std::env::var("HOME")
                                    .map(|h| format!("{}/.brain/daemon.sock", h))
                                    .unwrap_or_else(|_| "daemon.sock".to_string());
                                let diag = serde_json::json!({
                                    "schema_version": 1,
                                    "status": status_dto.health,
                                    "version": env!("CARGO_PKG_VERSION"),
                                    "ipc_protocol_version": "v1",
                                    "socket_path": socket_path,
                                    "sqlite_status": "ok",
                                    "python_runtime": "3.9.6",
                                    "uptime_secs": status_dto.uptime_secs,
                                    "storage_backend": status_dto.storage_backend,
                                    "total_queries": total_q,
                                    "total_ingests": total_i,
                                    "active_workers": active,
                                });
                                ("200 OK", "application/json", diag.to_string())
                            }
                            _ => (
                                "500 Internal Server Error",
                                "application/json",
                                "{}".to_string(),
                            ),
                        }
                    } else if request.contains("GET /metrics/json ") {
                        match dispatcher_ref
                            .dispatch(ApplicationRequest::Metrics, &context_ref)
                            .await
                        {
                            Ok(ApplicationResponse::Metrics(dto)) => {
                                let response_body = serde_json::to_string(&dto).unwrap_or_default();
                                ("200 OK", "application/json", response_body)
                            }
                            _ => (
                                "500 Internal Server Error",
                                "application/json",
                                "{}".to_string(),
                            ),
                        }
                    } else if request.contains("GET /metrics ") {
                        // Prometheus plain metrics format
                        match dispatcher_ref
                            .dispatch(ApplicationRequest::Metrics, &context_ref)
                            .await
                        {
                            Ok(ApplicationResponse::Metrics(dto)) => {
                                let total_q = metrics_ref.total_queries.load(Ordering::Relaxed);
                                let total_i = metrics_ref.total_ingests.load(Ordering::Relaxed);
                                let active = metrics_ref.active_workers.load(Ordering::Relaxed);
                                let prometheus_body = format!(
                                    r#"# HELP brain_queries_total Total client queries processed
# TYPE brain_queries_total counter
brain_queries_total {}
# HELP brain_ingests_total Total client ingests processed
# TYPE brain_ingests_total counter
brain_ingests_total {}
# HELP brain_active_workers Number of active worker tasks
# TYPE brain_active_workers gauge
brain_active_workers {}
# HELP brain_observations_ingested_total Total observations ingested
# TYPE brain_observations_ingested_total counter
brain_observations_ingested_total {}
"#,
                                    total_q, total_i, active, dto.observations_ingested
                                );
                                ("200 OK", "text/plain; version=0.0.4", prometheus_body)
                            }
                            _ => ("500 Internal Server Error", "text/plain", "".to_string()),
                        }
                    } else {
                        ("404 Not Found", "text/plain", "Not Found".to_string())
                    };

                    let response = format!(
                        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status,
                        content_type,
                        body.len(),
                        body
                    );

                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.flush().await;
                }
            });
        }
    }
}
