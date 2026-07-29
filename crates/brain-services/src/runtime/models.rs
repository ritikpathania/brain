#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub Uuid);

impl ExecutionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionHeader {
    pub execution_id: ExecutionId,
    pub parent_execution_id: Option<ExecutionId>,
    pub root_execution_id: ExecutionId,
    pub correlation_id: Option<String>,
    pub cause_id: Option<String>,
}

impl ExecutionHeader {
    pub fn new_root(execution_id: ExecutionId) -> Self {
        Self {
            execution_id,
            parent_execution_id: None,
            root_execution_id: execution_id,
            correlation_id: None,
            cause_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionFsmState {
    Created,
    Queued,
    Running,
    Checkpointing,
    Paused,
    Recovering,
    Completed,
    Failed,
    Cancelled,
}

impl ExecutionFsmState {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (ExecutionFsmState::Created, ExecutionFsmState::Queued)
                | (ExecutionFsmState::Queued, ExecutionFsmState::Running)
                | (ExecutionFsmState::Running, ExecutionFsmState::Checkpointing)
                | (ExecutionFsmState::Checkpointing, ExecutionFsmState::Running)
                | (ExecutionFsmState::Running, ExecutionFsmState::Paused)
                | (ExecutionFsmState::Paused, ExecutionFsmState::Running)
                | (ExecutionFsmState::Running, ExecutionFsmState::Recovering)
                | (ExecutionFsmState::Recovering, ExecutionFsmState::Running)
                | (ExecutionFsmState::Running, ExecutionFsmState::Completed)
                | (ExecutionFsmState::Running, ExecutionFsmState::Failed)
                | (ExecutionFsmState::Running, ExecutionFsmState::Cancelled)
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            ExecutionFsmState::Completed | ExecutionFsmState::Failed | ExecutionFsmState::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFsmState {
    Created,
    Waiting,
    Ready,
    Leased,
    Running,
    Checkpointing,
    Completed,
    Skipped,
    Failed,
    Cancelled,
}

impl TaskFsmState {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (TaskFsmState::Created, TaskFsmState::Waiting)
                | (TaskFsmState::Waiting, TaskFsmState::Ready)
                | (TaskFsmState::Ready, TaskFsmState::Leased)
                | (TaskFsmState::Leased, TaskFsmState::Running)
                | (TaskFsmState::Running, TaskFsmState::Checkpointing)
                | (TaskFsmState::Checkpointing, TaskFsmState::Running)
                | (TaskFsmState::Running, TaskFsmState::Completed)
                | (TaskFsmState::Running, TaskFsmState::Skipped)
                | (TaskFsmState::Running, TaskFsmState::Failed)
                | (TaskFsmState::Running, TaskFsmState::Cancelled)
                | (TaskFsmState::Failed, TaskFsmState::Waiting)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_identity_header() {
        let root_id = ExecutionId::new();
        let header = ExecutionHeader::new_root(root_id);
        assert_eq!(header.execution_id, root_id);
        assert_eq!(header.root_execution_id, root_id);
        assert!(header.parent_execution_id.is_none());
    }

    #[test]
    fn test_execution_fsm_transitions() {
        assert!(ExecutionFsmState::Created.can_transition_to(ExecutionFsmState::Queued));
        assert!(ExecutionFsmState::Running.can_transition_to(ExecutionFsmState::Recovering));
        assert!(ExecutionFsmState::Recovering.can_transition_to(ExecutionFsmState::Running));
        assert!(!ExecutionFsmState::Completed.can_transition_to(ExecutionFsmState::Running));
    }

    #[test]
    fn test_task_fsm_transitions() {
        assert!(TaskFsmState::Created.can_transition_to(TaskFsmState::Waiting));
        assert!(TaskFsmState::Waiting.can_transition_to(TaskFsmState::Ready));
        assert!(TaskFsmState::Ready.can_transition_to(TaskFsmState::Leased));
        assert!(TaskFsmState::Running.can_transition_to(TaskFsmState::Skipped));
        assert!(!TaskFsmState::Completed.can_transition_to(TaskFsmState::Ready));
    }
}
