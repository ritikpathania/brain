use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::net::UnixListener;
use tracing::error;

use crate::plugins::PluginRegistry;
use crate::server::handlers::handle_connection;
use crate::storage::duckdb::AnalyticsEvent;
use crate::{DaemonMetrics, GlobalState};

pub async fn start_uds_listener(
    listener: UnixListener,
    global_state: GlobalState,
    plugin_registry: Arc<PluginRegistry>,
    metrics: Arc<DaemonMetrics>,
    analytics_tx: tokio::sync::mpsc::UnboundedSender<AnalyticsEvent>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let state_ref = Arc::clone(&global_state);
                let registry_ref = Arc::clone(&plugin_registry);
                let connection_metrics = Arc::clone(&metrics);
                let connection_analytics_tx = analytics_tx.clone();

                tokio::spawn(async move {
                    connection_metrics
                        .active_workers
                        .fetch_add(1, Ordering::Relaxed);
                    if let Err(e) = handle_connection(
                        stream,
                        state_ref,
                        registry_ref,
                        connection_metrics.clone(),
                        connection_analytics_tx,
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
