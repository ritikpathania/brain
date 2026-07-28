#![allow(missing_docs)]

use crate::distributed::transport::TaskAssignment;
use crate::worker::context::*;
use crate::worker::models::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute(
        &self,
        assignment: &TaskAssignment,
        ctx: &TaskExecutionContext,
    ) -> Result<TaskResult, TaskExecutionError>;
}

pub trait TaskExecutorFactory: Send + Sync {
    fn create_executor(&self, assignment: &TaskAssignment) -> Arc<dyn TaskExecutor>;
}

pub struct InProcessExecutor;

impl Default for InProcessExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl InProcessExecutor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TaskExecutor for InProcessExecutor {
    async fn execute(
        &self,
        assignment: &TaskAssignment,
        ctx: &TaskExecutionContext,
    ) -> Result<TaskResult, TaskExecutionError> {
        if ctx.cancellation_token.is_cancelled() {
            return Err(TaskExecutionError::Cancelled);
        }

        let elapsed = ctx.started_at.elapsed().as_millis() as u64;

        Ok(TaskResult {
            task_id: assignment.task_id,
            output_ref: format!("artifact://outputs/{}/result.json", assignment.task_id.0),
            checkpoint_id: None,
            execution_time_ms: elapsed,
            metadata: HashMap::from([("executor".to_string(), "in_process".to_string())]),
        })
    }
}
