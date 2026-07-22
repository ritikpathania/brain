use crate::brain_runtime::InternalMetrics;
use crate::reflection::handler::ReflectionCommandHandler;
use crate::reflection::planner::ReflectionPlanner;
use crate::reflection::{ReflectionContext, ReflectionEngine};
use brain_config::schema::ReflectionSettings;
use brain_core::errors::BrainError;
use brain_core::repositories::Storage;
use brain_events::EventLog;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Background scheduler driving periodic and event-triggered reflection cycles under budget limits.
pub struct BackgroundReflectionScheduler {
    engine: Arc<ReflectionEngine>,
    storage: Arc<dyn Storage>,
    event_log: Arc<dyn EventLog>,
    settings: ReflectionSettings,
    metrics: Arc<InternalMetrics>,
    notify: Arc<tokio::sync::Notify>,
    cancel_token: CancellationToken,
    background_task: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
    last_processed_seq: AtomicU64,
    is_running: AtomicBool,
    is_dirty: AtomicBool,
}

impl BackgroundReflectionScheduler {
    /// Creates a new `BackgroundReflectionScheduler`.
    pub(crate) fn new(
        engine: Arc<ReflectionEngine>,
        storage: Arc<dyn Storage>,
        event_log: Arc<dyn EventLog>,
        settings: ReflectionSettings,
        metrics: Arc<InternalMetrics>,
    ) -> Self {
        Self {
            engine,
            storage,
            event_log,
            settings,
            metrics,
            notify: Arc::new(tokio::sync::Notify::new()),
            cancel_token: CancellationToken::new(),
            background_task: parking_lot::Mutex::new(None),
            last_processed_seq: AtomicU64::new(0),
            is_running: AtomicBool::new(false),
            is_dirty: AtomicBool::new(false),
        }
    }

    /// Returns reference to notify primitive for manually triggering cycles.
    pub fn notify(&self) -> &Arc<tokio::sync::Notify> {
        &self.notify
    }

    /// Triggers a single reflection cycle under single-flight level-triggered protection.
    pub fn run_cycle(&self, force: bool) -> Result<(), BrainError> {
        if self.cancel_token.is_cancelled() {
            return Ok(());
        }

        // Single-flight level-triggered protection: if already running, set dirty flag
        if self.is_running.swap(true, Ordering::AcqRel) {
            self.is_dirty.store(true, Ordering::Release);
            return Ok(());
        }

        let mut current_force = force;
        loop {
            let res = self.execute_cycle_internal(current_force);

            if self.cancel_token.is_cancelled() {
                self.is_running.store(false, Ordering::Release);
                return res;
            }

            // If triggered again during cycle execution, loop once more
            if self.is_dirty.swap(false, Ordering::AcqRel) {
                current_force = true;
                continue;
            }

            self.is_running.store(false, Ordering::Release);
            return res;
        }
    }

    fn execute_cycle_internal(&self, force: bool) -> Result<(), BrainError> {
        // 1. Check WAL event delta condition if not forced
        let last_seq = self.event_log.latest_sequence().unwrap_or(0);
        let prev_seq = self.last_processed_seq.load(Ordering::Acquire);

        if !force
            && self.settings.min_events_trigger() > 0
            && last_seq < prev_seq + self.settings.min_events_trigger()
        {
            return Ok(());
        }

        let start = Instant::now();
        let context = ReflectionContext {
            execution_id: uuid::Uuid::new_v4(),
            session_id: brain_domain::SessionId(ulid::Ulid::new()),
            cutoff_epoch: u64::MAX,
            max_nodes: self.settings.max_nodes_per_cycle(),
            time_budget_ms: self.settings.cycle_time_budget_ms(),
            cancellation_token: self.cancel_token.clone(),
        };

        // 2. Run read-only passes over transaction snapshot
        let findings = self.engine.reflect(&context)?;

        // 3. Evaluate decision plan via planner
        let planner = ReflectionPlanner::with_thresholds(
            self.settings.duplicate_confidence_threshold(),
            self.settings.link_suggestion_confidence_threshold(),
        );
        let plan = planner.plan(findings);

        // 4. Update metrics counters
        self.metrics
            .reflections_executed
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .reflection_findings_count
            .fetch_add(plan.findings_processed as u64, Ordering::Relaxed);
        self.metrics
            .reflection_commands_skipped
            .fetch_add(plan.skipped_findings.len() as u64, Ordering::Relaxed);

        // 5. Execute commands if auto_approve_merges is enabled
        if self.settings.auto_approve_merges() && !plan.commands.is_empty() {
            let commands = plan.commands;
            let handler = ReflectionCommandHandler::new();
            let mut executed_count = 0;

            let mut run_tx = |tx: &dyn brain_core::repositories::StorageTransaction| {
                for cmd in &commands {
                    let _ = handler.handle(tx, cmd.clone())?;
                    executed_count += 1;
                }
                Ok(())
            };

            self.storage.run_transaction(&mut run_tx)?;
            self.metrics
                .reflection_commands_executed
                .fetch_add(executed_count, Ordering::Relaxed);
        }

        // 6. Record timing and sequence checkpoint
        let elapsed = start.elapsed();
        self.metrics
            .last_reflection_duration_ns
            .store(elapsed.as_nanos() as u64, Ordering::Release);
        self.metrics
            .reflection_duration_ns
            .fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);
        self.last_processed_seq.store(last_seq, Ordering::Release);

        Ok(())
    }

    /// Starts the background reflection loop task.
    pub fn start(self: &Arc<Self>) -> Result<(), BrainError> {
        let mut handle_lock = self.background_task.lock();
        if handle_lock.is_some() {
            return Ok(());
        }

        if !self.settings.background_enabled() {
            return Ok(());
        }

        let scheduler = Arc::clone(self);
        let notify = self.notify.clone();
        let cancel = self.cancel_token.clone();
        let interval_secs = self.settings.interval_secs().max(1);

        let handle = tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(interval_secs));
            // First tick finishes immediately, skip first instant tick
            timer.tick().await;

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        break;
                    }
                    _ = timer.tick() => {
                        if let Err(e) = scheduler.run_cycle(false) {
                            tracing::warn!("Background Reflection tick error: {:?}", e);
                        }
                    }
                    _ = notify.notified() => {
                        if let Err(e) = scheduler.run_cycle(true) {
                            tracing::warn!("Notified Background Reflection tick error: {:?}", e);
                        }
                    }
                }
            }
        });

        *handle_lock = Some(handle);
        Ok(())
    }

    /// Halts background reflection processing gracefully.
    pub fn shutdown(&self) -> Result<(), BrainError> {
        self.cancel_token.cancel();
        self.notify.notify_one();

        let handle = self.background_task.lock().take();
        if let Some(h) = handle {
            h.abort();
        }
        Ok(())
    }
}
