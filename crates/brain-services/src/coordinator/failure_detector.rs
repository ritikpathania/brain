#![allow(missing_docs)]

use crate::coordinator::events::*;
use std::collections::HashMap;

pub struct FailureDetector {
    heartbeat_timeout_secs: u64,
    last_heartbeats: HashMap<String, u64>,
    lost_workers: HashMap<String, bool>,
}

impl FailureDetector {
    pub fn new(heartbeat_timeout_secs: u64) -> Self {
        Self {
            heartbeat_timeout_secs,
            last_heartbeats: HashMap::new(),
            lost_workers: HashMap::new(),
        }
    }

    pub fn record_heartbeat(&mut self, worker_id: String, timestamp: u64) {
        self.last_heartbeats.insert(worker_id, timestamp);
    }

    pub fn check_health(&mut self, worker_id: String, now: u64) -> Option<InternalEvent> {
        let last = self.last_heartbeats.get(&worker_id)?;
        let is_lost = *self.lost_workers.get(&worker_id).unwrap_or(&false);

        if now > last + self.heartbeat_timeout_secs {
            if !is_lost {
                self.lost_workers.insert(worker_id.clone(), true);
                return Some(InternalEvent::WorkerLost { worker_id });
            }
        } else if is_lost {
            self.lost_workers.insert(worker_id.clone(), false);
            return Some(InternalEvent::WorkerRecovered { worker_id });
        }
        None
    }
}
