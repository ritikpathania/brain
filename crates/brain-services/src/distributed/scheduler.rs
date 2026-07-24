#![allow(missing_docs)]

use crate::distributed::models::*;
use crate::distributed::registry::*;

pub trait SchedulingPolicy: Send + Sync {
    fn select_worker<'a>(&self, task_priority: u32, candidates: &'a [WorkerCandidate<'a>]) -> Option<WorkerCandidate<'a>>;
}

pub struct LeastLoadedPolicy;

impl SchedulingPolicy for LeastLoadedPolicy {
    fn select_worker<'a>(&self, _task_priority: u32, candidates: &'a [WorkerCandidate<'a>]) -> Option<WorkerCandidate<'a>> {
        candidates
            .iter()
            .filter(|w| w.status.is_healthy)
            .min_by(|a, b| {
                a.status
                    .current_load
                    .partial_cmp(&b.status.current_load)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }
}

pub struct WorkerScheduler<P: SchedulingPolicy> {
    registry: WorkerRegistry,
    policy: P,
}

impl<P: SchedulingPolicy> WorkerScheduler<P> {
    pub fn new(registry: WorkerRegistry, policy: P) -> Self {
        Self { registry, policy }
    }

    pub fn schedule_next_worker(&self, task_priority: u32) -> Option<RegisteredWorker> {
        let active = self.registry.list_active();
        let candidates: Vec<WorkerCandidate> = active
            .iter()
            .map(|w| WorkerCandidate {
                descriptor: &w.descriptor,
                status: &w.status,
            })
            .collect();

        let selected = self.policy.select_worker(task_priority, &candidates)?;
        self.registry.get(selected.descriptor.worker_id.as_str())
    }
}
