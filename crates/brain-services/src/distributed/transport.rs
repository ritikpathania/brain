#![allow(missing_docs)]

use crate::runtime::models::*;
use async_trait::async_trait;
use brain_domain::jobs::JobId;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Network connection error: {0}")]
    Network(String),
    #[error("Worker error: {0}")]
    Worker(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLease {
    pub lease_id: u64,
    pub lease_owner: String,
    pub lease_until: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub task_id: TaskId,
    pub execution_id: ExecutionId,
    pub job_id: JobId,
    pub input_ref: String,
    pub lease: TaskLease,
}

#[async_trait]
pub trait WorkerTransport: Send + Sync {
    async fn dispatch(&self, assignment: TaskAssignment) -> Result<(), TransportError>;
    async fn cancel(&self, task_id: TaskId) -> Result<(), TransportError>;
    async fn reconnect(&self) -> Result<(), TransportError>;
}

pub struct MockWorkerTransport {
    dispatched: Arc<Mutex<Vec<TaskAssignment>>>,
    cancelled: Arc<Mutex<Vec<TaskId>>>,
    should_fail_dispatch: AtomicBool,
}

impl Default for MockWorkerTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl MockWorkerTransport {
    pub fn new() -> Self {
        Self {
            dispatched: Arc::new(Mutex::new(Vec::new())),
            cancelled: Arc::new(Mutex::new(Vec::new())),
            should_fail_dispatch: AtomicBool::new(false),
        }
    }

    pub fn set_should_fail_dispatch(&self, fail: bool) {
        self.should_fail_dispatch.store(fail, Ordering::SeqCst);
    }

    pub fn dispatched_count(&self) -> usize {
        self.dispatched.lock().len()
    }

    pub fn last_dispatched(&self) -> Option<TaskAssignment> {
        self.dispatched.lock().last().cloned()
    }

    pub fn cancelled_count(&self) -> usize {
        self.cancelled.lock().len()
    }
}

#[async_trait]
impl WorkerTransport for MockWorkerTransport {
    async fn dispatch(&self, assignment: TaskAssignment) -> Result<(), TransportError> {
        if self.should_fail_dispatch.load(Ordering::SeqCst) {
            return Err(TransportError::Network("Emulated dispatch failure".to_string()));
        }
        self.dispatched.lock().push(assignment);
        Ok(())
    }

    async fn cancel(&self, task_id: TaskId) -> Result<(), TransportError> {
        self.cancelled.lock().push(task_id);
        Ok(())
    }

    async fn reconnect(&self) -> Result<(), TransportError> {
        Ok(())
    }
}
