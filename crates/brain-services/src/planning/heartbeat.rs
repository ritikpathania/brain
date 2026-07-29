//! `WorkerHeartbeatService` and `HeartbeatPolicy` Eviction Management (Phase 9 Milestone 9.2).
//!
//! ### Architectural Invariants:
//! 1. Policy Separation: `WorkerHeartbeatService` collects heartbeat observation timestamps ONLY; `HeartbeatPolicy` evaluates staleness.
//! 2. Stale Eviction: `evict_stale_workers` updates registry status of stale workers to `WorkerStatus::Offline`.

use crate::planning::worker_registry::{WorkerId, WorkerRegistry, WorkerStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Policy evaluating whether a worker is stale based on heartbeat observation timing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatPolicy {
    /// Maximum allowed duration in milliseconds without a heartbeat before marking stale.
    pub stale_timeout_ms: u64,
}

impl Default for HeartbeatPolicy {
    fn default() -> Self {
        Self {
            stale_timeout_ms: 10_000,
        }
    }
}

impl HeartbeatPolicy {
    /// Evaluates if a last heartbeat timestamp is stale relative to current time.
    pub fn is_stale(&self, last_heartbeat_ms: u64, current_time_ms: u64) -> bool {
        current_time_ms.saturating_sub(last_heartbeat_ms) > self.stale_timeout_ms
    }
}

/// Service collecting worker heartbeat timestamps and executing policy-based staleness evictions.
#[derive(Debug, Clone, Default)]
pub struct WorkerHeartbeatService {
    last_heartbeats: HashMap<WorkerId, u64>,
}

impl WorkerHeartbeatService {
    /// Instantiates a new `WorkerHeartbeatService`.
    pub fn new() -> Self {
        Self {
            last_heartbeats: HashMap::new(),
        }
    }

    /// Records a heartbeat observation for a worker node at specified timestamp.
    pub fn record_heartbeat(&mut self, worker_id: WorkerId, timestamp_ms: u64) {
        self.last_heartbeats.insert(worker_id, timestamp_ms);
    }

    /// Returns the last recorded heartbeat timestamp for a worker if present.
    pub fn get_last_heartbeat(&self, worker_id: WorkerId) -> Option<u64> {
        self.last_heartbeats.get(&worker_id).copied()
    }

    /// Evaluates all recorded heartbeats against `HeartbeatPolicy` and transitions stale workers to `WorkerStatus::Offline`.
    pub fn evict_stale_workers(
        &self,
        registry: &mut WorkerRegistry,
        policy: &HeartbeatPolicy,
        current_time_ms: u64,
    ) -> Vec<WorkerId> {
        let mut evicted = Vec::new();

        for (&worker_id, &last_hb) in &self.last_heartbeats {
            if policy.is_stale(last_hb, current_time_ms) {
                if let Some(worker) = registry.get_worker(worker_id) {
                    if worker.status != WorkerStatus::Offline {
                        let _ = registry.update_status(worker_id, WorkerStatus::Offline);
                        evicted.push(worker_id);
                    }
                }
            }
        }

        evicted.sort();
        evicted
    }
}
