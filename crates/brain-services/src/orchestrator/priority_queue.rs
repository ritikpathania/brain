use super::task::{OrchestratorTask, TaskId, TaskKind, TaskPriority};
use brain_core::errors::BrainError;
use std::collections::{HashSet, VecDeque};

/// Priority queue managing task ordering, dependency resolution, and tier-specific backpressure.
pub struct PriorityTaskQueue {
    tasks: VecDeque<OrchestratorTask>,
    completed_tasks: HashSet<TaskId>,
    capacity: usize,
}

impl PriorityTaskQueue {
    /// Creates a new `PriorityTaskQueue` with the specified bounded capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            tasks: VecDeque::new(),
            completed_tasks: HashSet::new(),
            capacity,
        }
    }

    /// Pushes a task into the queue according to priority rules and tier backpressure.
    pub fn push(&mut self, task: OrchestratorTask) -> Result<TaskId, BrainError> {
        let task_id = task.id;

        // 1. Coalescing for Normal Reflection tasks: if a Reflect task is already pending, coalesce
        if matches!(task.kind, TaskKind::Reflect { .. }) {
            for existing in &self.tasks {
                if matches!(existing.kind, TaskKind::Reflect { .. }) {
                    return Ok(existing.id);
                }
            }
        }

        // 2. Capacity & Backpressure Checks
        if self.tasks.len() >= self.capacity {
            match task.priority {
                TaskPriority::Low => {
                    // Discard low priority tasks on queue pressure
                    return Ok(task_id);
                }
                TaskPriority::Normal => {
                    // Try to drop a Low priority task if available
                    if let Some(pos) = self
                        .tasks
                        .iter()
                        .position(|t| t.priority == TaskPriority::Low)
                    {
                        self.tasks.remove(pos);
                    } else {
                        // Otherwise discard normal task
                        return Ok(task_id);
                    }
                }
                TaskPriority::High | TaskPriority::Critical => {
                    // Try to drop a Low or Normal priority task to make space
                    if let Some(pos) = self.tasks.iter().position(|t| {
                        t.priority == TaskPriority::Low || t.priority == TaskPriority::Normal
                    }) {
                        self.tasks.remove(pos);
                    } else {
                        return Err(BrainError::Validation {
                            message: format!(
                                "Orchestrator task queue at capacity ({}) for priority {:?}",
                                self.capacity, task.priority
                            ),
                        });
                    }
                }
            }
        }

        // 3. Insert in priority order (Critical > High > Normal > Low, then oldest first)
        let insert_idx = self
            .tasks
            .iter()
            .position(|t| {
                if task.priority > t.priority {
                    true
                } else if task.priority == t.priority {
                    task.created_at_unix_ms < t.created_at_unix_ms
                } else {
                    false
                }
            })
            .unwrap_or(self.tasks.len());

        self.tasks.insert(insert_idx, task);
        Ok(task_id)
    }

    /// Pops the highest-priority task whose dependencies have all completed.
    pub fn pop_ready(&mut self) -> Option<OrchestratorTask> {
        let ready_pos = self.tasks.iter().position(|t| {
            t.dependencies
                .iter()
                .all(|dep| self.completed_tasks.contains(dep))
        })?;

        self.tasks.remove(ready_pos)
    }

    /// Marks a task ID as completed, unblocking dependent downstream tasks.
    pub fn mark_completed(&mut self, id: TaskId) {
        self.completed_tasks.insert(id);
    }

    /// Checks if a task ID is marked as completed.
    pub fn is_completed(&self, id: &TaskId) -> bool {
        self.completed_tasks.contains(id)
    }

    /// Returns current number of pending tasks in the queue.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}
