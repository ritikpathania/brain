//! Reference in-memory implementations of CheckpointStore and SnapshotStore.

use crate::errors::StorageError;
use crate::traits::{CheckpointStore, SnapshotStore};
use std::collections::HashMap;
use std::sync::Mutex;

/// Reference in-memory thread-safe implementation of `CheckpointStore`.
#[derive(Default)]
pub struct InMemoryCheckpointStore {
    checkpoints: Mutex<HashMap<String, String>>,
}

impl InMemoryCheckpointStore {
    /// Creates a new `InMemoryCheckpointStore`.
    pub fn new() -> Self {
        Self {
            checkpoints: Mutex::new(HashMap::new()),
        }
    }
}

impl CheckpointStore for InMemoryCheckpointStore {
    fn save_checkpoint(&self, plan_id: &str, checkpoint_json: &str) -> Result<(), StorageError> {
        let mut cps = self
            .checkpoints
            .lock()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        cps.insert(plan_id.to_string(), checkpoint_json.to_string());
        Ok(())
    }

    fn load_checkpoint(&self, plan_id: &str) -> Result<Option<String>, StorageError> {
        let cps = self
            .checkpoints
            .lock()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        Ok(cps.get(plan_id).cloned())
    }
}

/// Reference in-memory thread-safe implementation of `SnapshotStore`.
#[derive(Default)]
pub struct InMemorySnapshotStore {
    snapshots: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemorySnapshotStore {
    /// Creates a new `InMemorySnapshotStore`.
    pub fn new() -> Self {
        Self {
            snapshots: Mutex::new(HashMap::new()),
        }
    }
}

impl SnapshotStore for InMemorySnapshotStore {
    fn save_snapshot(&self, snapshot_id: &str, data: &[u8]) -> Result<(), StorageError> {
        let mut snaps = self
            .snapshots
            .lock()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        snaps.insert(snapshot_id.to_string(), data.to_vec());
        Ok(())
    }

    fn load_snapshot(&self, snapshot_id: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let snaps = self
            .snapshots
            .lock()
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        Ok(snaps.get(snapshot_id).cloned())
    }
}
