//! Storage-agnostic CheckpointStore trait and InMemoryCheckpointStore.

use brain_domain::projection::*;
use std::collections::HashMap;

/// Trait for atomic checkpoint persistence.
pub trait CheckpointStore: Send + Sync {
    /// Saves a checkpoint atomically alongside projection state.
    fn save_checkpoint_atomic(&mut self, checkpoint: &Checkpoint) -> Result<(), ProjectionError>;
    /// Loads the latest checkpoint for a projection.
    fn load_checkpoint(&self, id: &ProjectionId) -> Result<Option<Checkpoint>, ProjectionError>;
    /// Resets checkpoint state for a projection.
    fn reset_projection(&mut self, id: &ProjectionId) -> Result<(), ProjectionError>;
}

/// In-memory CheckpointStore implementation for tests and volatile projections.
#[derive(Default)]
pub struct InMemoryCheckpointStore {
    checkpoints: HashMap<ProjectionId, Checkpoint>,
}

impl InMemoryCheckpointStore {
    /// Creates a new InMemoryCheckpointStore.
    pub fn new() -> Self {
        Self::default()
    }
}

impl CheckpointStore for InMemoryCheckpointStore {
    fn save_checkpoint_atomic(&mut self, checkpoint: &Checkpoint) -> Result<(), ProjectionError> {
        self.checkpoints
            .insert(checkpoint.projection_id.clone(), checkpoint.clone());
        Ok(())
    }

    fn load_checkpoint(&self, id: &ProjectionId) -> Result<Option<Checkpoint>, ProjectionError> {
        Ok(self.checkpoints.get(id).cloned())
    }

    fn reset_projection(&mut self, id: &ProjectionId) -> Result<(), ProjectionError> {
        self.checkpoints.remove(id);
        Ok(())
    }
}
