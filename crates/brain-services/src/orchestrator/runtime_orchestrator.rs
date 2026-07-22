use super::executor::TaskExecutor;
use super::priority_queue::PriorityTaskQueue;
use super::task::{OrchestratorTask, TaskId, TaskKind, TaskPriority, TaskStatus, TaskTraceRecord};
use crate::brain_runtime::InternalMetrics;
use brain_core::errors::BrainError;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Immutable snapshot of orchestrator internal state at a single point in time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrchestratorDiagnosticsSnapshot {
    /// Current number of pending tasks in the priority queue.
    pub pending_tasks_count: usize,
    /// Total cumulative tasks queued.
    pub tasks_queued: u64,
    /// Total cumulative tasks completed.
    pub tasks_completed: u64,
    /// Total cumulative tasks failed.
    pub tasks_failed: u64,
    /// Total cumulative tasks dropped under backpressure.
    pub tasks_dropped: u64,
    /// Duration of wait latency for the most recent task in milliseconds.
    pub last_task_wait_ms: u64,
    /// Duration of execution latency for the most recent task in milliseconds.
    pub last_task_exec_ms: u64,
    /// Details of the currently running task, if any.
    pub current_running_task: Option<TaskTraceRecord>,
    /// Bounded ring buffer of recent task execution trace history.
    pub task_history: Vec<TaskTraceRecord>,
}

/// Bounded single-loop background orchestrator governing system task scheduling and execution.
pub struct RuntimeOrchestrator {
    queue: Mutex<PriorityTaskQueue>,
    executor: Arc<dyn TaskExecutor>,
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
    current_running_task: Mutex<Option<TaskTraceRecord>>,
    task_history: Mutex<VecDeque<TaskTraceRecord>>,
    history_capacity: usize,
}

impl RuntimeOrchestrator {
    /// Creates a new standalone `RuntimeOrchestrator` for testing.
    pub fn new(executor: Arc<dyn TaskExecutor>, capacity: usize) -> Self {
        Self::with_history_capacity(executor, capacity, 100)
    }

    /// Creates a new standalone `RuntimeOrchestrator` with specified task history capacity.
    pub fn with_history_capacity(
        executor: Arc<dyn TaskExecutor>,
        capacity: usize,
        history_capacity: usize,
    ) -> Self {
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
            current_running_task: Mutex::new(None),
            task_history: Mutex::new(VecDeque::with_capacity(history_capacity)),
            history_capacity,
        }
    }

    /// Creates a new `RuntimeOrchestrator` with runtime metrics tracking.
    pub(crate) fn with_metrics(
        executor: Arc<dyn TaskExecutor>,
        metrics: Arc<InternalMetrics>,
        capacity: usize,
    ) -> Self {
        let mut inst = Self::new(executor, capacity);
        inst.metrics = Some(metrics);
        inst
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

    /// Captures an immutable atomic snapshot of current orchestrator diagnostics.
    pub fn diagnostics_snapshot(&self) -> OrchestratorDiagnosticsSnapshot {
        let current = self.current_running_task.lock().clone();
        let history: Vec<TaskTraceRecord> = self.task_history.lock().iter().cloned().collect();

        OrchestratorDiagnosticsSnapshot {
            pending_tasks_count: self.queue.lock().len(),
            tasks_queued: self.tasks_queued_count(),
            tasks_completed: self.tasks_completed_count(),
            tasks_failed: self.tasks_failed_count(),
            tasks_dropped: self.tasks_dropped_count(),
            last_task_wait_ms: self.last_task_wait_ms(),
            last_task_exec_ms: self.last_task_exec_ms(),
            current_running_task: current,
            task_history: history,
        }
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

        // Monotonic duration calculations immune to wall-clock drift
        let wait_ms = task
            .created_instant
            .map(|i| i.elapsed().as_millis() as u64)
            .unwrap_or_else(|| now_ms.saturating_sub(task.created_at_unix_ms));

        self.last_task_wait_ms.store(wait_ms, Ordering::Release);

        let mut trace = task.to_trace_record(TaskStatus::Running {
            started_at_unix_ms: now_ms,
        });
        trace.wait_duration_ms = wait_ms;

        *self.current_running_task.lock() = Some(trace.clone());

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

        let finish_now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        trace.exec_duration_ms = exec_ms;

        match result {
            Ok(()) => {
                self.tasks_completed.fetch_add(1, Ordering::Relaxed);
                trace.status = TaskStatus::Succeeded {
                    completed_at_unix_ms: finish_now_ms,
                    duration_ms: exec_ms,
                };
            }
            Err(ref e) => {
                tracing::warn!("Orchestrator task {:?} failed: {:?}", task_id, e);
                self.tasks_failed.fetch_add(1, Ordering::Relaxed);
                trace.status = TaskStatus::Failed {
                    failed_at_unix_ms: finish_now_ms,
                    error: e.to_string(),
                };
            }
        }

        *self.current_running_task.lock() = None;

        {
            let mut hist = self.task_history.lock();
            if hist.len() >= self.history_capacity {
                hist.pop_front();
            }
            hist.push_back(trace);
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

    /// Returns total number of tasks dropped under backpressure.
    pub fn tasks_dropped_count(&self) -> u64 {
        self.tasks_dropped.load(Ordering::Acquire)
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
