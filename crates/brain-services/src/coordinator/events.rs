#![allow(missing_docs)]

use crate::distributed::ingress::WorkerHeartbeat;
use crate::distributed::models::*;
use crate::runtime::models::*;
use crate::worker::models::*;
use brain_domain::jobs::JobId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExternalEvent {
    TaskEnqueued {
        task_id: TaskId,
        execution_id: ExecutionId,
        job_id: JobId,
        priority: u32,
    },
    WorkerRegistered {
        descriptor: WorkerDescriptor,
        status: WorkerStatus,
    },
    HeartbeatReceived {
        heartbeat: WorkerHeartbeat,
    },
    TaskExecutionEventReceived {
        event: TaskExecutionEvent,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InternalEvent {
    LeaseExpired { task_id: TaskId, lease_id: u64 },
    WorkerLost { worker_id: String },
    WorkerRecovered { worker_id: String },
    RetryDue { task_id: TaskId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CoordinatorEvent {
    External(Box<ExternalEvent>),
    Internal(InternalEvent),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_event_variants() {
        let task_id = TaskId::new();
        let exec_id = ExecutionId::new();
        let job_id = JobId(uuid::Uuid::new_v4());

        let ext = ExternalEvent::TaskEnqueued {
            task_id,
            execution_id: exec_id,
            job_id,
            priority: 1,
        };

        let ev = CoordinatorEvent::External(Box::new(ext));
        assert!(matches!(ev, CoordinatorEvent::External(_)));
    }
}
