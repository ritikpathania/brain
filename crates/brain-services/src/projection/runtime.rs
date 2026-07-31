//! ProjectionRuntime facade orchestrating registration, replay, scheduling, atomic checkpoint persistence, and graceful shutdown.

use crate::projection::instance::*;
use crate::projection::registry::*;
use crate::projection::replay::*;
use crate::projection::scheduler::*;
use crate::projection::store::*;
use brain_domain::bkf::events::FactEvent;
use brain_domain::projection::*;

/// Facade managing Phase 3 Projection Runtime operations.
pub struct ProjectionRuntime {
    registry: ProjectionRegistry,
    store: Box<dyn CheckpointStore>,
    scheduler: SequentialProjectionScheduler,
    running: bool,
}

impl ProjectionRuntime {
    /// Creates a new ProjectionRuntime with a CheckpointStore.
    pub fn new(store: Box<dyn CheckpointStore>) -> Self {
        Self {
            registry: ProjectionRegistry::new(),
            store,
            scheduler: SequentialProjectionScheduler::new(),
            running: true,
        }
    }

    /// Registers a projection instance.
    pub fn register_projection(
        &mut self,
        instance: ProjectionInstance,
    ) -> Result<(), ProjectionError> {
        self.registry.register(instance)
    }

    /// Dispatches an event live to registered projections and persists checkpoints atomically.
    pub fn dispatch_event(&mut self, event: &FactEvent, seq: u64) -> Result<(), ProjectionError> {
        if !self.running {
            return Err(ProjectionError::ReducerFailed {
                message: "Runtime is stopped".to_string(),
            });
        }

        self.scheduler
            .dispatch_event(&mut self.registry, event, seq)?;
        for instance in self.registry.instances_mut() {
            self.store.save_checkpoint_atomic(instance.checkpoint())?;
        }
        Ok(())
    }

    /// Catches up all projections to target watermark from event iterator.
    pub fn catchup_all<'a, I>(
        &mut self,
        event_iter: I,
        target_watermark: Watermark,
    ) -> Result<(), ProjectionError>
    where
        I: Iterator<Item = &'a FactEvent> + Clone,
    {
        for instance in self.registry.instances_mut() {
            ReplayEngine::replay_catchup(instance, event_iter.clone(), target_watermark)?;
            self.store.save_checkpoint_atomic(instance.checkpoint())?;
        }
        Ok(())
    }

    /// Executes graceful shutdown sequence: stop accepting events -> persist checkpoints -> set state to Stopped.
    pub fn shutdown(&mut self) -> Result<(), ProjectionError> {
        self.running = false;
        for instance in self.registry.instances_mut() {
            self.store.save_checkpoint_atomic(instance.checkpoint())?;
            instance.set_lifecycle(ProjectionLifecycle::Stopped);
        }
        Ok(())
    }
}
