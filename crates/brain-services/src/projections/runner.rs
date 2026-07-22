use crate::projections::{
    ProjectionId, ProjectionNotification, ProjectionNotificationBus, ReducerRegistry, StateReducer,
};
use brain_core::errors::BrainError;
use brain_events::{EventLog, SequenceNumber};
use brain_storage::SqliteProjectionCheckpointRepository;
use std::sync::Arc;

/// Orchestrator coordinating sequential event catch-up and rebuild loops for stateful reducers.
/// Kept for backward compatibility and test runners.
pub struct ProjectionRunner {
    event_log: Arc<dyn EventLog>,
    checkpoint_repo: Arc<SqliteProjectionCheckpointRepository>,
    notification_bus: Arc<ProjectionNotificationBus>,
    registry: ReducerRegistry,
}

impl ProjectionRunner {
    /// Creates a new `ProjectionRunner` instance.
    pub fn new(
        event_log: Arc<dyn EventLog>,
        checkpoint_repo: Arc<SqliteProjectionCheckpointRepository>,
        notification_bus: Arc<ProjectionNotificationBus>,
    ) -> Self {
        Self {
            event_log,
            checkpoint_repo,
            notification_bus,
            registry: ReducerRegistry::new(),
        }
    }

    /// Registers a stateful reducer.
    pub fn register(&self, reducer: Arc<dyn StateReducer>) -> Result<(), BrainError> {
        self.registry.register(reducer)
    }

    /// Runs incremental catch-up batches for all registered reducers.
    pub fn catch_up(&self) -> Result<(), BrainError> {
        let mut errors = Vec::new();
        let catch_up_res = self.registry.with_all(|id, reducer| {
            if let Err(e) = self.catch_up_reducer(id, reducer) {
                errors.push(format!("{:?}: {:?}", id, e));
            }
            Ok(())
        });

        catch_up_res?;

        if !errors.is_empty() {
            return Err(BrainError::Storage {
                message: format!("Errors occurred during catch-up: {}", errors.join("; ")),
                source: None,
            });
        }
        Ok(())
    }

    /// Rebuilds a specific projection from sequence 0.
    pub fn rebuild_projection(&self, id: ProjectionId) -> Result<(), BrainError> {
        let reducer = self.registry.get(id).ok_or_else(|| BrainError::Storage {
            message: format!("Reducer not found: {:?}", id),
            source: None,
        })?;

        let conn = self
            .checkpoint_repo
            .pool()
            .get()
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to get connection: {}", e),
                source: Some(Box::new(e)),
            })?;

        reducer.reset(&conn)?;

        let db_name = to_db_name(id);
        self.checkpoint_repo.save_checkpoint(db_name, 0)?;

        self.catch_up_reducer(id, reducer.as_ref())?;
        Ok(())
    }

    /// Rebuilds all registered projections.
    pub fn rebuild_all(&self) -> Result<(), BrainError> {
        let ids = self.registry.ids();
        for id in ids {
            self.rebuild_projection(id)?;
        }
        Ok(())
    }

    // Internal helper to batch catch up a specific reducer
    fn catch_up_reducer(
        &self,
        id: ProjectionId,
        reducer: &dyn StateReducer,
    ) -> Result<(), BrainError> {
        let db_name = to_db_name(id);
        let mut last_seq = self.checkpoint_repo.get_checkpoint(db_name)?;
        let mut advanced = false;

        let conn = self
            .checkpoint_repo
            .pool()
            .get()
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to get connection: {}", e),
                source: Some(Box::new(e)),
            })?;

        loop {
            let next_seq = last_seq + 1;
            let events = self.event_log.read_from(next_seq, 100)?;
            if events.is_empty() {
                break;
            }

            for envelope in events {
                let seq = envelope.sequence.ok_or_else(|| BrainError::Storage {
                    message: format!(
                        "Event missing sequence ID in EventLog for projection ID {:?}",
                        id
                    ),
                    source: None,
                })?;

                reducer.reduce(&conn, &envelope)?;
                self.checkpoint_repo.save_checkpoint(db_name, seq)?;
                last_seq = seq;
                advanced = true;
            }
        }

        if advanced {
            self.notification_bus.publish(ProjectionNotification {
                projection_id: id,
                sequence: SequenceNumber(last_seq),
            });
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
