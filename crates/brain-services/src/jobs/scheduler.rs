//! Background job scheduler with queueing, priority FIFO, cycles, and dependency resolution.

use super::executor::{
    JobExecutionContext, JobExecutionFailure, JobExecutionResult, JobExecutorRegistry,
};
use super::publisher::DomainEventPublisher;
use brain_domain::{Job, JobId, JobKind, JobPriority, JobState, JobTimestamp};
use parking_lot::RwLock;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

/// Monotonic enqueue ordinal to preserve submission order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnqueueOrdinal(pub u64);

/// Container mapping Job aggregate to scheduler queue metadata.
pub struct ScheduledJob {
    /// The encapsulated aggregate root.
    pub job: Job,
    /// Jobs that this job directly waits on.
    pub dependencies: BTreeSet<JobId>,
    /// Jobs that directly wait on this job.
    pub dependents: BTreeSet<JobId>,
    /// Order sequence number assigned during submission.
    pub enqueue_order: EnqueueOrdinal,
}

/// Unified error variants for scheduling failures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SchedulerError {
    /// Referenced job does not exist.
    #[error("Job {0:?} was not found")]
    UnknownJob(JobId),
    /// Job with identical ID already in storage.
    #[error("Job {0:?} already exists in queue")]
    DuplicateJob(JobId),
    /// Graph contains cyclic dependency paths.
    #[error("Dependency cycle detected")]
    DependencyCycle,
    /// No executor registered for task kind.
    #[error("No executor registered for kind {0:?}")]
    ExecutorNotRegistered(JobKind),
    /// Job is currently executing.
    #[error("Job {0:?} is already running")]
    AlreadyRunning(JobId),
    /// Job has already terminated.
    #[error("Job {0:?} has already completed")]
    AlreadyCompleted(JobId),
    /// Request to cancel a job failed.
    #[error("Executor failed to cancel job {0:?}")]
    CancellationFailed(JobId),
}

/// Internal comparator for ordering runnable jobs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RunnableJob {
    id: JobId,
    priority: JobPriority,
    enqueue_order: EnqueueOrdinal,
}

impl Ord for RunnableJob {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Critical is 0, Low is 3. We want Critical to sort BEFORE Low (ascending order).
        // Since BTreeSet returns ascending order, self.priority.cmp(other.priority) puts
        // Critical first!
        match self.priority.cmp(&other.priority) {
            std::cmp::Ordering::Equal => self.enqueue_order.cmp(&other.enqueue_order),
            ord => ord,
        }
    }
}

impl PartialOrd for RunnableJob {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

enum SchedulerCommand {
    Submit {
        job: Box<Job>,
        dependencies: BTreeSet<JobId>,
        reply: oneshot::Sender<Result<(), SchedulerError>>,
    },
    Cancel {
        job_id: JobId,
        reply: oneshot::Sender<Result<(), SchedulerError>>,
    },
    WorkerFinished {
        job_id: JobId,
        result: Result<JobExecutionResult, JobExecutionFailure>,
    },
}

struct SchedulerInner {
    jobs: HashMap<JobId, ScheduledJob>,
    ready_queue: BTreeSet<RunnableJob>,
    active_tokens: HashMap<JobId, CancellationToken>,
    next_ordinal: u64,
    max_concurrency: usize,
}

/// Thread-safe client handle managing job queues and execution loops.
#[derive(Clone)]
pub struct JobScheduler {
    tx: mpsc::Sender<SchedulerCommand>,
    jobs: Arc<RwLock<HashMap<JobId, Job>>>,
}

impl JobScheduler {
    /// Initialize a new JobScheduler with registry, event publisher, and concurrency limit.
    pub fn new(
        executor_registry: JobExecutorRegistry,
        event_publisher: Arc<dyn DomainEventPublisher>,
        max_concurrency: usize,
    ) -> Self {
        let (tx, rx) = mpsc::channel(100);
        let jobs = Arc::new(RwLock::new(HashMap::new()));
        let scheduler = Self {
            tx,
            jobs: jobs.clone(),
        };

        let inner = SchedulerInner {
            jobs: HashMap::new(),
            ready_queue: BTreeSet::new(),
            active_tokens: HashMap::new(),
            next_ordinal: 1,
            max_concurrency,
        };

        let tx_clone = scheduler.tx.clone();
        tokio::spawn(scheduler_actor_loop(
            rx,
            inner,
            executor_registry,
            jobs,
            tx_clone,
            event_publisher,
        ));

        scheduler
    }

    /// Submit a job to the scheduler.
    pub async fn submit(
        &self,
        job: Job,
        dependencies: BTreeSet<JobId>,
    ) -> Result<(), SchedulerError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(SchedulerCommand::Submit {
                job: Box::new(job),
                dependencies,
                reply: reply_tx,
            })
            .await
            .map_err(|_| SchedulerError::CancellationFailed(JobId(uuid::Uuid::nil())))?;
        reply_rx
            .await
            .map_err(|_| SchedulerError::CancellationFailed(JobId(uuid::Uuid::nil())))?
    }

    /// Request cancellation of a running job.
    pub async fn cancel(&self, id: JobId) -> Result<(), SchedulerError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(SchedulerCommand::Cancel {
                job_id: id,
                reply: reply_tx,
            })
            .await
            .map_err(|_| SchedulerError::CancellationFailed(id))?;
        reply_rx
            .await
            .map_err(|_| SchedulerError::CancellationFailed(id))?
    }

    /// Read-only check on the state of a job.
    pub fn get_job_state(&self, id: JobId) -> Option<JobState> {
        self.jobs.read().get(&id).map(|j| j.state())
    }

    /// Read-only clone of a job aggregate.
    pub fn get_job(&self, id: JobId) -> Option<Job> {
        self.jobs.read().get(&id).cloned()
    }
}

async fn scheduler_actor_loop(
    mut rx: mpsc::Receiver<SchedulerCommand>,
    mut inner: SchedulerInner,
    registry: JobExecutorRegistry,
    shared_jobs: Arc<RwLock<HashMap<JobId, Job>>>,
    self_tx: mpsc::Sender<SchedulerCommand>,
    event_publisher: Arc<dyn DomainEventPublisher>,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            SchedulerCommand::Submit {
                job,
                dependencies,
                reply,
            } => {
                let job = *job;
                let job_id = job.id();
                let job_kind = job.kind();

                if inner.jobs.contains_key(&job_id) {
                    let _ = reply.send(Err(SchedulerError::DuplicateJob(job_id)));
                    continue;
                }

                if registry.get(job_kind).is_none() {
                    let _ = reply.send(Err(SchedulerError::ExecutorNotRegistered(job_kind)));
                    continue;
                }

                // Check for cycles
                if has_dependency_cycle(&inner.jobs, job_id, &dependencies) {
                    let _ = reply.send(Err(SchedulerError::DependencyCycle));
                    continue;
                }

                let ordinal = EnqueueOrdinal(inner.next_ordinal);
                inner.next_ordinal += 1;

                // Collect unresolved dependencies. We only count dependencies that exist in scheduler
                // and are NOT completed yet.
                let mut unresolved_dependencies = BTreeSet::new();
                for &dep in &dependencies {
                    if let Some(sj) = inner.jobs.get(&dep) {
                        if sj.job.state() != JobState::Completed {
                            unresolved_dependencies.insert(dep);
                        }
                    } else {
                        // Dangling dependency counts as unresolved
                        unresolved_dependencies.insert(dep);
                    }
                }

                let runnable = unresolved_dependencies.is_empty();

                let mut scheduled = ScheduledJob {
                    job,
                    dependencies: unresolved_dependencies,
                    dependents: BTreeSet::new(),
                    enqueue_order: ordinal,
                };

                // Drain and publish job events (e.g. JobCreated)
                for event in scheduled.job.drain_events() {
                    event_publisher.publish(event);
                }

                // Sync to shared_jobs map
                {
                    shared_jobs.write().insert(job_id, scheduled.job.clone());
                }

                // Link dependents on parent jobs
                for &dep in &dependencies {
                    if let Some(parent) = inner.jobs.get_mut(&dep) {
                        parent.dependents.insert(job_id);
                    }
                }

                let runnable_job = RunnableJob {
                    id: job_id,
                    priority: scheduled.job.priority(),
                    enqueue_order: ordinal,
                };

                inner.jobs.insert(job_id, scheduled);

                if runnable {
                    inner.ready_queue.insert(runnable_job);
                }

                let _ = reply.send(Ok(()));
                dispatch_ready_jobs(
                    &mut inner,
                    &registry,
                    &shared_jobs,
                    &self_tx,
                    &event_publisher,
                );
            }

            SchedulerCommand::Cancel { job_id, reply } => {
                let mut cancel_immediate = false;
                let mut ordinal_opt = None;
                let mut priority_opt = None;

                if let Some(sj) = inner.jobs.get_mut(&job_id) {
                    if is_terminal(sj.job.state()) {
                        let _ = reply.send(Err(SchedulerError::AlreadyCompleted(job_id)));
                        continue;
                    }

                    if let Some(token) = inner.active_tokens.get(&job_id) {
                        token.cancel();
                    } else {
                        cancel_immediate = true;
                        ordinal_opt = Some(sj.enqueue_order);
                        priority_opt = Some(sj.job.priority());

                        let ts = JobTimestamp(current_unix_timestamp());
                        let _ = sj.job.cancel(ts);

                        for event in sj.job.drain_events() {
                            event_publisher.publish(event);
                        }

                        // Sync state
                        shared_jobs.write().insert(job_id, sj.job.clone());
                    }
                } else {
                    let _ = reply.send(Err(SchedulerError::UnknownJob(job_id)));
                    continue;
                }

                if cancel_immediate {
                    if let (Some(ord), Some(prio)) = (ordinal_opt, priority_opt) {
                        let runnable = RunnableJob {
                            id: job_id,
                            priority: prio,
                            enqueue_order: ord,
                        };
                        inner.ready_queue.remove(&runnable);
                    }
                    cascade_cancellation(job_id, &mut inner, &shared_jobs, &event_publisher);
                }

                let _ = reply.send(Ok(()));
            }

            SchedulerCommand::WorkerFinished { job_id, result } => {
                inner.active_tokens.remove(&job_id);

                let mut completed_dependents = Vec::new();
                let mut failed_or_cancelled = false;

                if let Some(sj) = inner.jobs.get_mut(&job_id) {
                    let ts = JobTimestamp(current_unix_timestamp());
                    let was_cancelled = result.is_err();

                    if was_cancelled {
                        let _ = sj.job.cancel(ts);
                        failed_or_cancelled = true;
                    } else if let Ok(res) = result {
                        // Write logs and artifacts
                        for (log_ts, log_msg) in res.logs {
                            let _ = sj.job.append_log(log_ts, log_msg);
                        }
                        for art in res.artifacts {
                            let _ = sj.job.produce_artifact(art.id, art.kind, art.payload);
                        }
                        let _ = sj.job.complete(ts);

                        completed_dependents = sj.dependents.iter().cloned().collect();
                    }

                    for event in sj.job.drain_events() {
                        event_publisher.publish(event);
                    }

                    // Sync state
                    shared_jobs.write().insert(job_id, sj.job.clone());
                }

                // If completed, release dependents
                if !completed_dependents.is_empty() {
                    let mut readied = Vec::new();
                    for dep_id in completed_dependents {
                        if let Some(dep_sj) = inner.jobs.get_mut(&dep_id) {
                            dep_sj.dependencies.remove(&job_id);
                            if dep_sj.dependencies.is_empty()
                                && dep_sj.job.state() == JobState::Pending
                            {
                                readied.push(RunnableJob {
                                    id: dep_id,
                                    priority: dep_sj.job.priority(),
                                    enqueue_order: dep_sj.enqueue_order,
                                });
                            }
                        }
                    }
                    for r_job in readied {
                        inner.ready_queue.insert(r_job);
                    }
                }

                // If failed or cancelled, cascade cancellation
                if failed_or_cancelled {
                    cascade_cancellation(job_id, &mut inner, &shared_jobs, &event_publisher);
                }

                dispatch_ready_jobs(
                    &mut inner,
                    &registry,
                    &shared_jobs,
                    &self_tx,
                    &event_publisher,
                );
            }
        }
    }
}

fn dispatch_ready_jobs(
    inner: &mut SchedulerInner,
    registry: &JobExecutorRegistry,
    shared_jobs: &Arc<RwLock<HashMap<JobId, Job>>>,
    self_tx: &mpsc::Sender<SchedulerCommand>,
    event_publisher: &Arc<dyn DomainEventPublisher>,
) {
    while inner.active_tokens.len() < inner.max_concurrency {
        // Pop highest priority ready job
        let next_runnable = inner.ready_queue.iter().next().cloned();
        if let Some(r_job) = next_runnable {
            inner.ready_queue.remove(&r_job);

            if let Some(sj) = inner.jobs.get_mut(&r_job.id) {
                let token = CancellationToken::new();
                inner.active_tokens.insert(r_job.id, token.clone());

                let ts = JobTimestamp(current_unix_timestamp());
                let _ = sj.job.start(ts);

                for event in sj.job.drain_events() {
                    event_publisher.publish(event);
                }

                {
                    shared_jobs.write().insert(r_job.id, sj.job.clone());
                }

                let worker_tx = self_tx.clone();
                let registry_clone = registry.clone();
                let job_id = r_job.id;
                let job_kind = sj.job.kind();

                tokio::spawn(async move {
                    let executor = registry_clone.get(job_kind);
                    let ctx = JobExecutionContext::new(token);

                    let res = if let Some(exec) = executor {
                        exec.execute(job_id, &ctx).await
                    } else {
                        Err(JobExecutionFailure("No executor registered".to_string()))
                    };

                    let _ = worker_tx
                        .send(SchedulerCommand::WorkerFinished {
                            job_id,
                            result: res,
                        })
                        .await;
                });
            }
        } else {
            break;
        }
    }
}

fn cascade_cancellation(
    failed_id: JobId,
    inner: &mut SchedulerInner,
    shared_jobs: &Arc<RwLock<HashMap<JobId, Job>>>,
    event_publisher: &Arc<dyn DomainEventPublisher>,
) {
    let mut to_cancel = Vec::new();
    if let Some(sj) = inner.jobs.get(&failed_id) {
        for &dep_id in &sj.dependents {
            to_cancel.push(dep_id);
        }
    }

    let ts = JobTimestamp(current_unix_timestamp());
    while let Some(job_id) = to_cancel.pop() {
        if let Some(sj) = inner.jobs.get_mut(&job_id) {
            if !is_terminal(sj.job.state()) {
                let _ = sj.job.cancel(ts);

                for event in sj.job.drain_events() {
                    event_publisher.publish(event);
                }

                {
                    shared_jobs.write().insert(job_id, sj.job.clone());
                }
                let runnable = RunnableJob {
                    id: job_id,
                    priority: sj.job.priority(),
                    enqueue_order: sj.enqueue_order,
                };
                inner.ready_queue.remove(&runnable);

                // Add nested dependents
                for &nested_dep in &sj.dependents {
                    to_cancel.push(nested_dep);
                }
            }
        }
    }
}

fn is_terminal(state: JobState) -> bool {
    matches!(
        state,
        JobState::Completed | JobState::Failed | JobState::Cancelled
    )
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn has_dependency_cycle(
    jobs: &HashMap<JobId, ScheduledJob>,
    start_id: JobId,
    new_deps: &BTreeSet<JobId>,
) -> bool {
    let mut visited = HashSet::new();
    let mut path = HashSet::new();

    fn dfs(
        id: JobId,
        jobs: &HashMap<JobId, ScheduledJob>,
        start_id: JobId,
        new_deps: &BTreeSet<JobId>,
        visited: &mut HashSet<JobId>,
        path: &mut HashSet<JobId>,
    ) -> bool {
        if path.contains(&id) {
            return true;
        }
        if visited.contains(&id) {
            return false;
        }

        path.insert(id);

        let empty_deps = BTreeSet::new();
        let deps = if id == start_id {
            new_deps
        } else if let Some(sj) = jobs.get(&id) {
            &sj.dependencies
        } else {
            &empty_deps
        };

        for &dep in deps {
            if dfs(dep, jobs, start_id, new_deps, visited, path) {
                return true;
            }
        }

        path.remove(&id);
        visited.insert(id);
        false
    }

    dfs(start_id, jobs, start_id, new_deps, &mut visited, &mut path)
}
