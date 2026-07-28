#![allow(missing_docs)]

use crate::distributed::transport::TaskAssignment;
use crate::worker::context::*;
use crate::worker::executor::*;
use crate::worker::models::*;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

pub struct TimeoutExecutor {
    inner: Arc<dyn TaskExecutor>,
    timeout: Duration,
}

impl TimeoutExecutor {
    pub fn new(inner: Arc<dyn TaskExecutor>, timeout: Duration) -> Self {
        Self { inner, timeout }
    }
}

#[async_trait]
impl TaskExecutor for TimeoutExecutor {
    async fn execute(
        &self,
        assignment: &TaskAssignment,
        ctx: &TaskExecutionContext,
    ) -> Result<TaskResult, TaskExecutionError> {
        match tokio::time::timeout(self.timeout, self.inner.execute(assignment, ctx)).await {
            Ok(res) => res,
            Err(_) => Err(TaskExecutionError::Timeout(self.timeout)),
        }
    }
}

pub struct RetryExecutor {
    inner: Arc<dyn TaskExecutor>,
    max_retries: u32,
}

impl RetryExecutor {
    pub fn new(inner: Arc<dyn TaskExecutor>, max_retries: u32) -> Self {
        Self { inner, max_retries }
    }
}

#[async_trait]
impl TaskExecutor for RetryExecutor {
    async fn execute(
        &self,
        assignment: &TaskAssignment,
        ctx: &TaskExecutionContext,
    ) -> Result<TaskResult, TaskExecutionError> {
        let mut attempts = 0;
        loop {
            match self.inner.execute(assignment, ctx).await {
                Ok(res) => return Ok(res),
                Err(err) => {
                    attempts += 1;
                    if attempts > self.max_retries || matches!(err, TaskExecutionError::Cancelled) {
                        return Err(err);
                    }
                }
            }
        }
    }
}
