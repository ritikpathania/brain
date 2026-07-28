#![allow(missing_docs)]

use crate::coordinator::queue::*;
use crate::distributed::models::*;
use crate::distributed::scheduler::*;
use crate::distributed::transport::*;
use crate::runtime::models::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulingDecision {
    Assign(TaskAssignment),
    Defer(TaskId),
    Reject(TaskId),
}

pub struct WorkerSnapshot<'a> {
    pub candidates: &'a [WorkerCandidate<'a>],
}

pub struct SchedulingEngine<P: SchedulingPolicy> {
    policy: P,
}

impl<P: SchedulingPolicy> SchedulingEngine<P> {
    pub fn new(policy: P) -> Self {
        Self { policy }
    }

    pub fn schedule<'a>(
        &self,
        queue: &'a QueueSnapshot,
        workers: &'a WorkerSnapshot<'a>,
    ) -> Vec<SchedulingDecision> {
        let mut decisions = Vec::new();

        for task in &queue.ready_tasks {
            if let Some(candidate) = self.policy.select_worker(task.priority, workers.candidates) {
                let assignment = TaskAssignment {
                    task_id: task.task_id,
                    execution_id: task.execution_id,
                    job_id: task.job_id,
                    input_ref: format!("artifact://inputs/{}/input.json", task.task_id.0),
                    lease: TaskLease {
                        lease_id: 1,
                        lease_owner: candidate.descriptor.worker_id.clone(),
                        lease_until: 1000,
                    },
                };
                decisions.push(SchedulingDecision::Assign(assignment));
            } else {
                decisions.push(SchedulingDecision::Defer(task.task_id));
            }
        }

        decisions
    }
}
