use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::net::UnixListener;
use tracing::error;

use brain_services::BrainRuntime;

use crate::server::handlers::handle_connection;
use crate::DaemonMetrics;

pub async fn start_uds_listener(
    listener: UnixListener,
    metrics: Arc<DaemonMetrics>,
    brain_runtime: Arc<BrainRuntime>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let connection_metrics = Arc::clone(&metrics);
                let runtime_ref = Arc::clone(&brain_runtime);

                tokio::spawn(async move {
                    connection_metrics
                        .active_workers
                        .fetch_add(1, Ordering::Relaxed);
                    if let Err(e) =
                        handle_connection(stream, connection_metrics.clone(), runtime_ref).await
                    {
                        error!("Connection handler encountered error: {}", e);
                    }
                    connection_metrics
                        .active_workers
                        .fetch_sub(1, Ordering::Relaxed);
                });
            }
            Err(e) => {
                error!("Failed to accept incoming Unix stream: {}", e);
            }
        }
    }
}
