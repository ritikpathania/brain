#![allow(missing_docs)]

use crate::distributed::models::*;
use crate::runtime::sqlite_repository::*;
use crate::runtime::models::TaskId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngressError {
    #[error("Stale lease {0} for task {1:?}")]
    StaleLease(u64, String),
    #[error("Worker is unhealthy or unresponsive")]
    UnhealthyWorker,
    #[error("Storage error: {0}")]
    Storage(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskLeaseItem {
    pub task_id: TaskId,
    pub lease_id: u64,
}

#[derive(Debug, Clone)]
pub struct WorkerHeartbeat {
    pub worker_id: String,
    pub timestamp: u64,
    pub active_leases: Vec<TaskLeaseItem>,
    pub status: WorkerStatus,
}

pub struct CoordinatorIngressGate {
    _repo: SqliteExecutionRepository,
}

impl CoordinatorIngressGate {
    pub fn new(repo: SqliteExecutionRepository) -> Self {
        Self { _repo: repo }
    }

    pub fn process_heartbeat(&self, heartbeat: &WorkerHeartbeat) -> Result<(), IngressError> {
        if !heartbeat.status.is_healthy {
            return Err(IngressError::UnhealthyWorker);
        }
        Ok(())
    }
}
