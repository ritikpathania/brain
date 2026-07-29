#![allow(missing_docs)]

use crate::runtime::models::*;
use brain_domain::jobs::JobId;
use serde::{Deserialize, Serialize};
use std::collections::BinaryHeap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueError {
    #[error("Queue full: depth limit {0} reached")]
    QueueFull(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskNode {
    pub task_id: TaskId,
    pub execution_id: ExecutionId,
    pub job_id: JobId,
    pub priority: u32,
    pub enqueued_at: u64,
}

impl Ord for TaskNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialOrd for TaskNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct QueueSnapshot {
    pub ready_tasks: Vec<TaskNode>,
}

pub struct QueueManager {
    max_depth: usize,
    heap: BinaryHeap<TaskNode>,
}

impl QueueManager {
    pub fn new(max_depth: usize) -> Self {
        Self {
            max_depth,
            heap: BinaryHeap::new(),
        }
    }

    pub fn enqueue(
        &mut self,
        task_id: TaskId,
        execution_id: ExecutionId,
        job_id: JobId,
        priority: u32,
    ) -> Result<(), QueueError> {
        if self.heap.len() >= self.max_depth {
            return Err(QueueError::QueueFull(self.max_depth));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.heap.push(TaskNode {
            task_id,
            execution_id,
            job_id,
            priority,
            enqueued_at: now,
        });

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        let mut sorted: Vec<TaskNode> = self.heap.iter().cloned().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.priority));
        QueueSnapshot {
            ready_tasks: sorted,
        }
    }
}
