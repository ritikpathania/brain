use crate::DaemonMetrics;
use brain_application::dispatcher::RequestDispatcher;
use std::sync::Arc;

/// Delegates the HTTP health, readiness, and metrics server to the HTTP transport adapter.
pub async fn start_health_server(
    metrics: Arc<DaemonMetrics>,
    dispatcher: Arc<RequestDispatcher>,
    health_port: String,
    socket_path: String,
) {
    crate::transport::http::handlers::start_health_server(
        metrics,
        dispatcher,
        health_port,
        socket_path,
    )
    .await;
}
