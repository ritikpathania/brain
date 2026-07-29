//! Storage contracts for EventStore, CheckpointStore, and SnapshotStore.

use crate::errors::StorageError;

/// Trait defining persistent storage operations for plan checkpoints.
pub trait CheckpointStore: Send + Sync {
    /// Saves a serialized plan checkpoint record.
    fn save_checkpoint(&self, plan_id: &str, checkpoint_json: &str) -> Result<(), StorageError>;

    /// Loads a serialized plan checkpoint record by plan ID.
    fn load_checkpoint(&self, plan_id: &str) -> Result<Option<String>, StorageError>;
}

/// Trait defining storage operations for binary state snapshots.
pub trait SnapshotStore: Send + Sync {
    /// Saves a binary state snapshot under a snapshot ID.
    fn save_snapshot(&self, snapshot_id: &str, data: &[u8]) -> Result<(), StorageError>;

    /// Loads a binary state snapshot by snapshot ID.
    fn load_snapshot(&self, snapshot_id: &str) -> Result<Option<Vec<u8>>, StorageError>;
}
