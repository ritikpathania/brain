use super::executor::TaskExecutor;
use super::priority_queue::PriorityTaskQueue;
use super::task::{OrchestratorTask, TaskId, TaskKind, TaskPriority};
use crate::brain_runtime::InternalMetrics;
use brain_core::errors::BrainError;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Bounded single-loop background orchestrator governing system task scheduling and execution.
pub struct RuntimeOrchestrator {
    queue: Mutex<PriorityTaskQueue>,
    executor: Arc<dyn TaskExecutor>,
    #[allow(dead_code)]
    metrics: Option<Arc<InternalMetrics>>,
    notify: Arc<tokio::sync::Notify>,
    cancel_token: CancellationToken,
    background_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    tasks_queued: AtomicU64,
    tasks_completed: AtomicU64,
    tasks_failed: AtomicU64,
    tasks_dropped: AtomicU64,
    last_task_wait_ms: AtomicU64,
    last_task_exec_ms: AtomicU64,
}

impl RuntimeOrchestrator {
    /// Creates a new standalone `RuntimeOrchestrator` for testing.
    pub fn new(executor: Arc<dyn TaskExecutor>, capacity: usize) -> Self {
        Self {
            queue: Mutex::new(PriorityTaskQueue::new(capacity)),
            executor,
            metrics: None,
            notify: Arc::new(tokio::sync::Notify::new()),
            cancel_token: CancellationToken::new(),
            background_task: Mutex::new(None),
            tasks_queued: AtomicU64::new(0),
            tasks_completed: AtomicU64::new(0),
            tasks_failed: AtomicU64::new(0),
            tasks_dropped: AtomicU64::new(0),
            last_task_wait_ms: AtomicU64::new(0),
            last_task_exec_ms: AtomicU64::new(0),
        }
    }

    /// Creates a new `RuntimeOrchestrator` with runtime metrics tracking.
    pub(crate) fn with_metrics(
        executor: Arc<dyn TaskExecutor>,
        metrics: Arc<InternalMetrics>,
        capacity: usize,
    ) -> Self {
        Self {
            queue: Mutex::new(PriorityTaskQueue::new(capacity)),
            executor,
            metrics: Some(metrics),
            notify: Arc::new(tokio::sync::Notify::new()),
            cancel_token: CancellationToken::new(),
            background_task: Mutex::new(None),
            tasks_queued: AtomicU64::new(0),
            tasks_completed: AtomicU64::new(0),
            tasks_failed: AtomicU64::new(0),
            tasks_dropped: AtomicU64::new(0),
            last_task_wait_ms: AtomicU64::new(0),
            last_task_exec_ms: AtomicU64::new(0),
        }
    }

    /// Enqueues a declarative `OrchestratorTask` into the priority queue.
    pub fn schedule(&self, task: OrchestratorTask) -> Result<TaskId, BrainError> {
        let mut q = self.queue.lock();
        let prev_len = q.len();
        let task_id = q.push(task)?;
        if q.len() > prev_len {
            self.tasks_queued.fetch_add(1, Ordering::Relaxed);
        } else if q.len() == prev_len {
            self.tasks_dropped.fetch_add(1, Ordering::Relaxed);
        }
        self.notify.notify_one();
        Ok(task_id)
    }

    /// Helper to schedule a task by `TaskKind` and `TaskPriority`.
    pub fn submit(&self, kind: TaskKind, priority: TaskPriority) -> Result<TaskId, BrainError> {
        self.schedule(OrchestratorTask::new(kind, priority))
    }

    /// Processes a single ready task from the queue synchronously (for event loop or unit testing).
    pub fn tick(&self) -> Result<Option<TaskId>, BrainError> {
        let task = {
            let mut q = self.queue.lock();
            q.pop_ready()
        };

        let task = match task {
            Some(t) => t,
            None => return Ok(None),
        };

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let wait_ms = now_ms.saturating_sub(task.created_at_unix_ms);
        self.last_task_wait_ms.store(wait_ms, Ordering::Release);

        let start_time = Instant::now();
        let task_id = task.id;

        // Subsystem Failure Isolation: Subsystem execution errors do not crash the orchestrator
        let result = self.executor.execute(&task);
        let exec_ms = start_time.elapsed().as_millis() as u64;
        self.last_task_exec_ms.store(exec_ms, Ordering::Release);

        {
            let mut q = self.queue.lock();
            q.mark_completed(task_id);
        }

        match result {
            Ok(()) => {
                self.tasks_completed.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                tracing::warn!("Orchestrator task {:?} failed: {:?}", task_id, e);
                self.tasks_failed.fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(Some(task_id))
    }

    /// Starts the background orchestrator single event loop task.
    pub fn start(self: &Arc<Self>) -> Result<(), BrainError> {
        let mut handle_lock = self.background_task.lock();
        if handle_lock.is_some() {
            return Ok(());
        }

        let orchestrator = Arc::clone(self);
        let notify = self.notify.clone();
        let cancel = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            loop {
                // Process all ready tasks in the queue before sleeping
                while let Ok(Some(_)) = orchestrator.tick() {
                    if cancel.is_cancelled() {
                        break;
                    }
                }

                tokio::select! {
                    _ = cancel.cancelled() => {
                        break;
                    }
                    _ = notify.notified() => {
                        // Woken up by new task schedule notification
                    }
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {
                        // Periodic heartbeat wake up
                    }
                }
            }
        });

        *handle_lock = Some(handle);
        Ok(())
    }

    /// Halts background orchestrator processing cleanly.
    pub fn shutdown(&self) -> Result<(), BrainError> {
        self.cancel_token.cancel();
        self.notify.notify_one();

        let handle = self.background_task.lock().take();
        if let Some(h) = handle {
            h.abort();
        }
        Ok(())
    }

    /// Returns the number of currently pending tasks in the queue.
    pub fn pending_tasks_count(&self) -> usize {
        self.queue.lock().len()
    }

    /// Returns total number of tasks queued.
    pub fn tasks_queued_count(&self) -> u64 {
        self.tasks_queued.load(Ordering::Acquire)
    }

    /// Returns total number of tasks successfully completed.
    pub fn tasks_completed_count(&self) -> u64 {
        self.tasks_completed.load(Ordering::Acquire)
    }

    /// Returns total number of tasks failed.
    pub fn tasks_failed_count(&self) -> u64 {
        self.tasks_failed.load(Ordering::Acquire)
    }

    /// Returns last task wait time in milliseconds.
    pub fn last_task_wait_ms(&self) -> u64 {
        self.last_task_wait_ms.load(Ordering::Acquire)
    }

    /// Returns last task execution duration in milliseconds.
    pub fn last_task_exec_ms(&self) -> u64 {
        self.last_task_exec_ms.load(Ordering::Acquire)
    }
}
