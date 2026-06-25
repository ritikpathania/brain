use std::sync::atomic::AtomicU64;

#[derive(Default)]
pub struct DaemonMetrics {
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub total_ingests: AtomicU64,
    pub total_queries: AtomicU64,
    pub active_workers: AtomicU64,
    pub sum_query_latency_us: AtomicU64,
    pub sum_ingest_latency_us: AtomicU64,
    pub sum_extraction_latency_us: AtomicU64,
    pub sum_sqlite_latency_us: AtomicU64,
    pub sum_ipc_latency_us: AtomicU64,
    pub stm_queue_depth: AtomicU64,
}

impl DaemonMetrics {
    pub fn new() -> Self {
        Self::default()
    }
}
