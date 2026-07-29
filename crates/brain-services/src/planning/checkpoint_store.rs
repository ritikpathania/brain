//! Storage-Agnostic `CheckpointStore` Trait and `InMemoryCheckpointStore` Reference Implementation (Phase 8 Milestone 8.3).
//!
//! ### Architectural Invariants & Consistency Guarantees:
//! 1. Storage-Agnostic Interface: `CheckpointStore` decouples supervisor logic from persistence backends.
//! 2. Atomic Save: Checkpoint storage operation is atomic; duplicate checkpoint IDs are rejected.
//! 3. Immutable Load: Loaded `ExecutionCheckpoint` artifacts are strictly immutable.
//! 4. Deterministic Listing: `list_checkpoints` returns checkpoints ordered deterministically by timestamp.

use crate::planning::execution_runtime::ExecutionId;
use crate::planning::supervision::{CheckpointId, ExecutionCheckpoint, SupervisionError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Trait defining storage contracts for persistent `ExecutionCheckpoint` artifacts.
pub trait CheckpointStore: Send + Sync {
    /// Atomically persists an `ExecutionCheckpoint`. Rejects duplicate checkpoint IDs.
    fn save_checkpoint(&self, checkpoint: &ExecutionCheckpoint) -> Result<(), SupervisionError>;

    /// Loads an immutable `ExecutionCheckpoint` by ID.
    fn load_checkpoint(
        &self,
        checkpoint_id: CheckpointId,
    ) -> Result<ExecutionCheckpoint, SupervisionError>;

    /// Lists all checkpoints associated with an `ExecutionId`, ordered deterministically by timestamp.
    fn list_checkpoints(
        &self,
        execution_id: ExecutionId,
    ) -> Result<Vec<ExecutionCheckpoint>, SupervisionError>;
}

/// In-memory reference implementation of `CheckpointStore`.
#[derive(Debug, Clone, Default)]
pub struct InMemoryCheckpointStore {
    storage: Arc<Mutex<HashMap<CheckpointId, ExecutionCheckpoint>>>,
}

impl InMemoryCheckpointStore {
    /// Instantiates a new `InMemoryCheckpointStore`.
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl CheckpointStore for InMemoryCheckpointStore {
    fn save_checkpoint(&self, checkpoint: &ExecutionCheckpoint) -> Result<(), SupervisionError> {
        let mut map = self
            .storage
            .lock()
            .map_err(|e| SupervisionError::CorruptedCheckpoint(format!("Lock error: {}", e)))?;

        if map.contains_key(&checkpoint.checkpoint_id) {
            return Err(SupervisionError::CorruptedCheckpoint(format!(
                "Duplicate checkpoint ID '{}' rejected",
                checkpoint.checkpoint_id
            )));
        }

        map.insert(checkpoint.checkpoint_id, checkpoint.clone());
        Ok(())
    }

    fn load_checkpoint(
        &self,
        checkpoint_id: CheckpointId,
    ) -> Result<ExecutionCheckpoint, SupervisionError> {
        let map = self
            .storage
            .lock()
            .map_err(|e| SupervisionError::CorruptedCheckpoint(format!("Lock error: {}", e)))?;

        map.get(&checkpoint_id).cloned().ok_or_else(|| {
            SupervisionError::CheckpointMismatch(format!(
                "Checkpoint ID '{}' not found",
                checkpoint_id
            ))
        })
    }

    fn list_checkpoints(
        &self,
        execution_id: ExecutionId,
    ) -> Result<Vec<ExecutionCheckpoint>, SupervisionError> {
        let map = self
            .storage
            .lock()
            .map_err(|e| SupervisionError::CorruptedCheckpoint(format!("Lock error: {}", e)))?;

        let mut list: Vec<ExecutionCheckpoint> = map
            .values()
            .filter(|chk| chk.execution_id == execution_id)
            .cloned()
            .collect();

        // Deterministic sorting by timestamp
        list.sort_by_key(|c| c.timestamp_ms);

        Ok(list)
    }
}
