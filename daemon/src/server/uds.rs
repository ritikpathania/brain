use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::net::UnixListener;
use tracing::error;

use crate::transport::uds::handlers::handle_connection;
use crate::DaemonMetrics;
use brain_application::dispatcher::RequestDispatcher;
use brain_application::BrainApplication;

pub async fn start_uds_listener(
    listener: UnixListener,
    metrics: Arc<DaemonMetrics>,
    dispatcher: Arc<RequestDispatcher>,
    app: Arc<BrainApplication>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let connection_metrics = Arc::clone(&metrics);
                let dispatcher_ref = Arc::clone(&dispatcher);
                let app_ref = Arc::clone(&app);

                tokio::spawn(async move {
                    connection_metrics
                        .active_workers
                        .fetch_add(1, Ordering::Relaxed);
                    if let Err(e) = handle_connection(
                        stream,
                        connection_metrics.clone(),
                        dispatcher_ref,
                        app_ref,
                    )
                    .await
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
