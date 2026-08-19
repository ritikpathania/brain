use crate::projections::{
    ProjectionConfig, ProjectionId, ProjectionMetadata, ProjectionScheduler, ReducerRegistry,
    StateReducer,
};
use brain_core::errors::BrainError;
use brain_events::EventLog;
use brain_storage::{
    ProjectionMetadataRecord, ProjectionStatus, SqliteProjectionMetadataRepository,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

/// Sequential executor implementation of the ProjectionScheduler.
/// Handles execution policies, transaction atomicity, and rebuild lifecycle state transitions.
pub struct SequentialScheduler {
    registry: ReducerRegistry,
    metadata_repo: Arc<SqliteProjectionMetadataRepository>,
    event_log: Arc<dyn EventLog>,
    config: ProjectionConfig,
    pool: brain_storage::r2d2::Pool<brain_storage::connection::SqliteConnectionManager>,
}

impl SequentialScheduler {
    /// Creates a new `SequentialScheduler` instance.
    pub fn new(
        registry: ReducerRegistry,
        metadata_repo: Arc<SqliteProjectionMetadataRepository>,
        event_log: Arc<dyn EventLog>,
        config: ProjectionConfig,
        pool: brain_storage::r2d2::Pool<brain_storage::connection::SqliteConnectionManager>,
    ) -> Self {
        Self {
            registry,
            metadata_repo,
            event_log,
            config,
            pool,
        }
    }

    /// Internal catch-up method that runs on a given database connection.
    fn catch_up_reducer(
        &self,
        conn: &mut brain_storage::Connection,
        reducer: &dyn StateReducer,
        record: &mut ProjectionMetadataRecord,
    ) -> Result<(), BrainError> {
        let mut last_seq = record.last_sequence;

        loop {
            let next_seq = last_seq + 1;
            let events = self.event_log.read_from(next_seq, self.config.batch_size)?;
            if events.is_empty() {
                break;
            }

            // Execute batch processing inside a single transaction context
            let tx = conn.transaction().map_err(|e| BrainError::Storage {
                message: format!("Failed to create transaction: {}", e),
                source: Some(Box::new(e)),
            })?;

            let result = || -> Result<(), BrainError> {
                for envelope in events {
                    let seq = envelope.sequence.ok_or_else(|| BrainError::Storage {
                        message: "Sequence number missing in WAL event log".to_string(),
                        source: None,
                    })?;

                    reducer.reduce(&tx, &envelope)?;
                    last_seq = seq;
                }

                // Update checkpoint sequence metadata in the same transaction
                record.last_sequence = last_seq;
                record.status = ProjectionStatus::Active;
                record.last_error = None;
                record.updated_at = current_time_secs();
                self.metadata_repo.save_metadata(&tx, record)?;
                Ok(())
            }();

            match result {
                Ok(_) => {
                    tx.commit().map_err(|e| BrainError::Storage {
                        message: format!("Failed to commit transaction: {}", e),
                        source: Some(Box::new(e)),
                    })?;
                }
                Err(err) => {
                    let _ = tx.rollback();
                    return Err(err);
                }
            }
        }

        // Save final Idle status when all caught up
        let tx = conn.transaction().map_err(|e| BrainError::Storage {
            message: format!("Failed to create final transaction: {}", e),
            source: Some(Box::new(e)),
        })?;
        record.status = ProjectionStatus::Idle;
        record.updated_at = current_time_secs();
        self.metadata_repo.save_metadata(&tx, record)?;
        tx.commit().map_err(|e| BrainError::Storage {
            message: format!("Failed to commit final transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }
}

impl ProjectionScheduler for SequentialScheduler {
    fn catch_up_all(&self) -> Result<(), BrainError> {
        let ids = self.registry.ids();
        for id in ids {
            if let Err(e) = self.catch_up_projection(id) {
                tracing::error!("Failed to catch up projection {:?}: {:?}", id, e);
            }
        }
        Ok(())
    }

    fn catch_up_projection(&self, id: ProjectionId) -> Result<(), BrainError> {
        let reducer = self.registry.get(id).ok_or_else(|| BrainError::Storage {
            message: format!("No reducer registered for projection {:?}", id),
            source: None,
        })?;

        let mut conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to acquire connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let db_name = to_db_name(id);

        // Start transaction to get/initialize metadata
        let tx = conn.transaction().map_err(|e| BrainError::Storage {
            message: format!("Failed to begin transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut record = match self.metadata_repo.get_metadata(&tx, db_name)? {
            Some(existing) => existing,
            None => {
                let initial = ProjectionMetadataRecord {
                    name: db_name.to_string(),
                    version: reducer.version(),
                    last_sequence: 0,
                    status: ProjectionStatus::Idle,
                    last_error: None,
                    updated_at: current_time_secs(),
                };
                self.metadata_repo.save_metadata(&tx, &initial)?;
                initial
            }
        };

        // If version skew is detected, trigger a clean rebuild
        if record.version != reducer.version() {
            tracing::info!(
                "Version mismatch for projection {:?} (stored: {}, code: {}). Triggering rebuild.",
                id,
                record.version,
                reducer.version()
            );
            tx.rollback().map_err(|e| BrainError::Storage {
                message: format!("Failed to rollback: {}", e),
                source: Some(Box::new(e)),
            })?;
            return self.rebuild_projection(id);
        }

        // Commit transaction before processing catch-up loop (which manages its own transaction boundaries)
        tx.commit().map_err(|e| BrainError::Storage {
            message: format!("Failed to commit metadata transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        // If projection was failed, we still try to resume but under the failed state until it succeeds
        let catch_up_result = self.catch_up_reducer(&mut conn, reducer.as_ref(), &mut record);

        if let Err(e) = catch_up_result {
            // Log failure inside metadata store
            record.status = ProjectionStatus::Failed;
            record.last_error = Some(e.to_string());
            record.updated_at = current_time_secs();

            // Save the failure state using the existing connection
            let save_res = || -> Result<(), BrainError> {
                let tx = conn.transaction().map_err(|e| BrainError::Storage {
                    message: e.to_string(),
                    source: Some(Box::new(e)),
                })?;
                self.metadata_repo.save_metadata(&tx, &record)?;
                tx.commit().map_err(|e| BrainError::Storage {
                    message: e.to_string(),
                    source: Some(Box::new(e)),
                })?;
                Ok(())
            }();
            if let Err(save_err) = save_res {
                tracing::error!("Failed to save projection failure metadata: {:?}", save_err);
            }
            return Err(e);
        }

        Ok(())
    }

    fn rebuild_projection(&self, id: ProjectionId) -> Result<(), BrainError> {
        let reducer = self.registry.get(id).ok_or_else(|| BrainError::Storage {
            message: format!("No reducer registered for projection {:?}", id),
            source: None,
        })?;

        let mut conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to acquire connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let db_name = to_db_name(id);

        // Stage 1: Setup rebuilding state and reset read model
        let tx = conn.transaction().map_err(|e| BrainError::Storage {
            message: format!("Failed to start transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut record = ProjectionMetadataRecord {
            name: db_name.to_string(),
            version: reducer.version(),
            last_sequence: 0,
            status: ProjectionStatus::Rebuilding,
            last_error: None,
            updated_at: current_time_secs(),
        };

        // Reset the read model state
        if let Err(e) = reducer.reset(&tx) {
            record.status = ProjectionStatus::Failed;
            record.last_error = Some(e.to_string());
            record.updated_at = current_time_secs();
            let _ = self.metadata_repo.save_metadata(&tx, &record);
            let _ = tx.commit();
            return Err(e);
        }
        self.metadata_repo.save_metadata(&tx, &record)?;
        tx.commit().map_err(|e| BrainError::Storage {
            message: format!("Failed to commit setup transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        // Stage 2: Replay catch-up
        let catch_up_result = self.catch_up_reducer(&mut conn, reducer.as_ref(), &mut record);

        if let Err(e) = catch_up_result {
            record.status = ProjectionStatus::Failed;
            record.last_error = Some(e.to_string());
            record.updated_at = current_time_secs();

            if let Ok(mut error_conn) = self.pool.get() {
                let save_res = || -> Result<(), BrainError> {
                    let tx = error_conn.transaction().map_err(|e| BrainError::Storage {
                        message: e.to_string(),
                        source: Some(Box::new(e)),
                    })?;
                    self.metadata_repo.save_metadata(&tx, &record)?;
                    tx.commit().map_err(|e| BrainError::Storage {
                        message: e.to_string(),
                        source: Some(Box::new(e)),
                    })?;
                    Ok(())
                }();
                if let Err(save_err) = save_res {
                    tracing::error!("Failed to save projection failure metadata: {:?}", save_err);
                }
            }
            return Err(e);
        }

        // Stage 3: Mark as idle (caught up)
        let tx = conn.transaction().map_err(|e| BrainError::Storage {
            message: format!("Failed to begin active transaction: {}", e),
            source: Some(Box::new(e)),
        })?;
        record.status = ProjectionStatus::Idle;
        record.updated_at = current_time_secs();
        self.metadata_repo.save_metadata(&tx, &record)?;
        tx.commit().map_err(|e| BrainError::Storage {
            message: format!("Failed to commit final status: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    fn rebuild_all(&self) -> Result<(), BrainError> {
        let ids = self.registry.ids();
        for id in ids {
            self.rebuild_projection(id)?;
        }
        Ok(())
    }

    fn list_metadata(&self) -> Result<Vec<ProjectionMetadata>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to acquire connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut results = Vec::new();
        let ids = self.registry.ids();

        for id in ids {
            let db_name = to_db_name(id);
            if let Some(record) = self.metadata_repo.get_metadata(&conn, db_name)? {
                results.push(ProjectionMetadata {
                    name: record.name,
                    version: record.version,
                    last_sequence: record.last_sequence,
                    status: record.status,
                    last_error: record.last_error,
                    updated_at: record.updated_at,
                });
            } else {
                let reducer = self.registry.get(id).unwrap();
                results.push(ProjectionMetadata {
                    name: db_name.to_string(),
                    version: reducer.version(),
                    last_sequence: 0,
                    status: ProjectionStatus::Idle,
                    last_error: None,
                    updated_at: current_time_secs(),
                });
            }
        }

        Ok(results)
    }
}

/// Lifecycle runtime owning the background execution task loop.
pub struct SchedulerRuntime {
    scheduler: Arc<dyn ProjectionScheduler>,
    notify: Arc<tokio::sync::Notify>,
    cancel_token: CancellationToken,
    background_task: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl SchedulerRuntime {
    /// Creates a new `SchedulerRuntime` instance.
    pub fn new(scheduler: Arc<dyn ProjectionScheduler>) -> Self {
        Self {
            scheduler,
            notify: Arc::new(tokio::sync::Notify::new()),
            cancel_token: CancellationToken::new(),
            background_task: parking_lot::Mutex::new(None),
        }
    }

    /// Exposes reference to notify primitive for signalling ticks.
    pub fn notify(&self) -> &Arc<tokio::sync::Notify> {
        &self.notify
    }

    /// Starts the background scheduler catch-up loop.
    pub fn start(&self) -> Result<(), BrainError> {
        let mut handle_lock = self.background_task.lock();
        if handle_lock.is_some() {
            return Ok(());
        }

        let scheduler = self.scheduler.clone();
        let notify = self.notify.clone();
        let cancel = self.cancel_token.clone();

        // 1. Initial catch-up on startup to handle crash recovery / pending events
        if let Err(e) = scheduler.catch_up_all() {
            tracing::error!("Projection Engine: Startup catch-up failed: {:?}", e);
        }

        // 2. Spawn background sequential processing loop
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        break;
                    }
                    _ = notify.notified() => {
                        if let Err(e) = scheduler.catch_up_all() {
                            tracing::error!("Projection Engine: Background catch-up failed: {:?}", e);
                        }
                    }
                }
            }
        });

        *handle_lock = Some(handle);
        Ok(())
    }

    /// Stops the background scheduler catch-up loop gracefully.
    pub fn shutdown(&self) -> Result<(), BrainError> {
        self.cancel_token.cancel();
        self.notify.notify_one();

        let handle = self.background_task.lock().take();
        if let Some(h) = handle {
            // We run it synchronously inside a block or block the thread
            // But since this is a shutdown, we don't strictly block async loops if we don't want to.
            // Under normal tokio conditions, abort or drop is clean.
            h.abort();
        }
        Ok(())
    }
}

fn to_db_name(id: ProjectionId) -> &'static str {
    match id {
        ProjectionId::Jobs => "jobs",
        ProjectionId::Sessions => "sessions",
        ProjectionId::Search => "search",
        ProjectionId::Retrieval => "retrieval",
        ProjectionId::TestA => "test_a",
        ProjectionId::TestB => "test_b",
        ProjectionId::TestC => "test_c",
    }
}

fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
