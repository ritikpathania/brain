#![allow(missing_docs)]

use crate::runtime::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub output_ref: String,
    pub checkpoint_id: Option<String>,
    pub execution_time_ms: u64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Error)]
pub enum TaskExecutionError {
    #[error("Task execution cancelled")]
    Cancelled,
    #[error("Task execution timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Resource unavailable: {0}")]
    ResourceUnavailable(String),
    #[error("Artifact error: {0}")]
    ArtifactError(String),
    #[error("Checkpoint error: {0}")]
    CheckpointError(String),
    #[error("Internal executor error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskExecutionEvent {
    Started { task_id: TaskId, timestamp: u64 },
    Progress { task_id: TaskId, percentage: u8, message: Option<String> },
    CheckpointSaved { task_id: TaskId, checkpoint_id: String },
    Completed { task_id: TaskId, result: TaskResult },
    Failed { task_id: TaskId, error: String },
}
