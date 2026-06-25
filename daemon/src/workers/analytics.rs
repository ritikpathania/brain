use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::error;

use crate::storage::duckdb::{AnalyticsDatabase, AnalyticsEvent};

pub async fn start_analytics_worker(
    mut rx: UnboundedReceiver<AnalyticsEvent>,
    analytics_db: Arc<AnalyticsDatabase>,
) {
    while let Some(event) = rx.recv().await {
        if let Err(e) = analytics_db.record_event(event) {
            error!(
                component = "analytics",
                "Failed to record event in DuckDB: {}", e
            );
        }
    }
}
