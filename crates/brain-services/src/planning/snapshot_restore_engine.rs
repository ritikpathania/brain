//! Decoupled `SnapshotRestoreEngine` for Follower State Restoration (Phase 13 Milestone 13.2).
//!
//! ### Architectural Invariants:
//! 1. Decoupled Consensus vs Restoration: `ConsensusEngine` handles RPC validation; `SnapshotRestoreEngine` performs state payload restoration.
//! 2. Checksum Verification: Validates SHA-256 payload checksums prior to restoring projection state.
//! 3. Replay Target Integration: Restores state into targets implementing `RestoreFromSnapshot<S>`.

use crate::planning::durable_event_store::SequenceNumber;
use crate::planning::event_publisher::EventPublishError;
use crate::planning::recovery_engine::RestoreFromSnapshot;
use crate::planning::snapshot_store::{LogSnapshot, SnapshotCodec};

/// Engine restoring snapshot state payloads into projection targets.
pub struct SnapshotRestoreEngine;

impl SnapshotRestoreEngine {
    /// Restores a snapshot artifact into a target projection, returning the restored sequence number.
    pub fn restore_snapshot<S, T, C>(
        snapshot: &LogSnapshot,
        target: &mut T,
        codec: &C,
    ) -> Result<SequenceNumber, EventPublishError>
    where
        T: RestoreFromSnapshot<S>,
        C: SnapshotCodec<S>,
    {
        if !snapshot.verify_checksum() {
            return Err(EventPublishError::StorageError(
                "Snapshot checksum verification failed during follower restoration".to_string(),
            ));
        }

        let state_instance = codec.decode(&snapshot.state_payload)?;
        target.restore_snapshot(&state_instance);

        Ok(snapshot.snapshot_sequence)
    }
}
