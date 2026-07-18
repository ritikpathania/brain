use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

#[derive(Debug, Clone)]
struct Xorshift {
    state: u32,
}

impl Xorshift {
    fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}

pub struct LatencyReservoir {
    samples: Vec<u64>,
    count: usize,
    rng: Xorshift,
}

impl LatencyReservoir {
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(512),
            count: 0,
            rng: Xorshift::new(12345),
        }
    }

    pub fn observe(&mut self, latency_us: u64) {
        self.count += 1;
        if self.samples.len() < 512 {
            self.samples.push(latency_us);
        } else {
            let j = (self.rng.next_u32() as usize) % self.count;
            if j < 512 {
                self.samples[j] = latency_us;
            }
        }
    }

    pub fn percentiles(&self) -> (u64, u64, u64) {
        if self.samples.is_empty() {
            return (0, 0, 0);
        }
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let len = sorted.len();

        let p50 = sorted[len * 50 / 100];
        let p95 = sorted[len * 95 / 100];
        let p99 = sorted[len * 99 / 100];

        (p50, p95, p99)
    }
}

impl Default for LatencyReservoir {
    fn default() -> Self {
        Self::new()
    }
}

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

    // Sprint 7 — BrainRuntime parity counters
    // Sampled from independent atomics; treat individual scrapes as transient snapshots.
    // Use long-term trends and rates for migration decisions, not single samples.
    pub runtime_ingest_attempts: AtomicU64,
    pub runtime_ingest_successes: AtomicU64,
    pub runtime_ingest_failures: AtomicU64,
    pub runtime_ingest_latency_us: AtomicU64, // sum, same unit as sum_ingest_latency_us

    // Sprint 8 — stage timing sums
    pub runtime_canonicalization_latency_us: AtomicU64,
    pub runtime_reflection_latency_us: AtomicU64,
    pub runtime_dispatch_latency_us: AtomicU64,

    // Sprint 8 — reservoir sampler for percentile estimation
    pub runtime_latency_reservoir: Mutex<LatencyReservoir>,
}

impl DaemonMetrics {
    pub fn new() -> Self {
        Self::default()
    }
}
