#![allow(missing_docs)]

use crate::runtime::models::TaskId;
use crate::worker::models::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("Artifact error: {0}")]
    Io(String),
    #[error("Invalid artifact reference: {0}")]
    InvalidRef(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactKind {
    Input,
    Output,
    Log,
    Checkpoint,
}

#[async_trait]
pub trait ArtifactStore: Send + Sync {
    async fn stage_input(&self, input_ref: &str) -> Result<PathBuf, TaskExecutionError>;
    async fn publish_artifact(&self, task_id: TaskId, kind: ArtifactKind, local_path: &PathBuf) -> Result<String, TaskExecutionError>;
}

pub struct LocalFilesystemArtifactStore {
    base_dir: PathBuf,
}

impl LocalFilesystemArtifactStore {
    pub fn new(base_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&base_dir).ok();
        Self { base_dir }
    }
}

#[async_trait]
impl ArtifactStore for LocalFilesystemArtifactStore {
    async fn stage_input(&self, input_ref: &str) -> Result<PathBuf, TaskExecutionError> {
        let rel_path = input_ref.trim_start_matches("artifact://");
        let target = self.base_dir.join(rel_path);
        if target.exists() {
            Ok(target)
        } else {
            Err(TaskExecutionError::ArtifactError(format!("Input file not found: {}", input_ref)))
        }
    }

    async fn publish_artifact(&self, task_id: TaskId, kind: ArtifactKind, local_path: &PathBuf) -> Result<String, TaskExecutionError> {
        let file_name = local_path.file_name().ok_or_else(|| TaskExecutionError::ArtifactError("Invalid filename".to_string()))?;
        let kind_dir = match kind {
            ArtifactKind::Input => "inputs",
            ArtifactKind::Output => "outputs",
            ArtifactKind::Log => "logs",
            ArtifactKind::Checkpoint => "checkpoints",
        };

        let dest_dir = self.base_dir.join(kind_dir).join(task_id.0.to_string());
        std::fs::create_dir_all(&dest_dir).map_err(|e| TaskExecutionError::ArtifactError(e.to_string()))?;

        let dest = dest_dir.join(file_name);
        std::fs::copy(local_path, &dest).map_err(|e| TaskExecutionError::ArtifactError(e.to_string()))?;

        let rel = format!("{}/{}/{}", kind_dir, task_id.0, file_name.to_string_lossy());
        Ok(format!("artifact://{}", rel))
    }
}
