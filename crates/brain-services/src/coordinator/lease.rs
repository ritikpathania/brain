#![allow(missing_docs)]

use crate::coordinator::events::*;
use crate::distributed::transport::TaskLease;
use crate::runtime::models::*;
use std::collections::HashMap;

pub struct LeaseManager {
    lease_duration_secs: u64,
    active_leases: HashMap<TaskId, TaskLease>,
}

impl LeaseManager {
    pub fn new(lease_duration_secs: u64) -> Self {
        Self {
            lease_duration_secs,
            active_leases: HashMap::new(),
        }
    }

    pub fn allocate_lease(&mut self, task_id: TaskId, worker_id: &str, now: u64) -> TaskLease {
        let lease = TaskLease {
            lease_id: 1,
            lease_owner: worker_id.to_string(),
            lease_until: now + self.lease_duration_secs,
        };
        self.active_leases.insert(task_id, lease.clone());
        lease
    }

    pub fn sweep_expired(&mut self, now: u64) -> Vec<InternalEvent> {
        let mut expired = Vec::new();
        for (task_id, lease) in &self.active_leases {
            if lease.lease_until < now {
                expired.push(InternalEvent::LeaseExpired {
                    task_id: *task_id,
                    lease_id: lease.lease_id,
                });
            }
        }
        expired
    }
}
