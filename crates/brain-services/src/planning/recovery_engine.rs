//! Orchestrated `RecoveryEngine` for Incremental State Recovery (`SnapshotThenReplay`) (Phase 12 Milestone 12.2).
//!
//! ### Architectural Invariants:
//! 1. Incremental Recovery Pipeline: Restores latest valid snapshot from `SnapshotStore` (if available), then replays log entries starting from `snapshot_sequence + 1`.
//! 2. Checksum Integrity Verification: Validates SHA-256 payload checksums before restoring snapshots.
//! 3. Equivalence Invariant: `RestoreSnapshot(S) + Replay(S+1..N) == Replay(1..N)`.

use crate::planning::durable_event_store::{EventLog, SequenceNumber};
use crate::planning::event_publisher::EventPublishError;
use crate::planning::log_replay_engine::{LogReplayEngine, ReplayTarget};
use crate::planning::snapshot_store::{SnapshotCodec, SnapshotStore};

/// Target contract allowing projections to restore from a snapshot state instance.
pub trait RestoreFromSnapshot<S> {
    /// Restores projection state from a snapshot instance.
    fn restore_snapshot(&mut self, state: &S);
}

/// Orchestrated recovery engine driving incremental snapshot + log replay recovery.
pub struct RecoveryEngine;

impl RecoveryEngine {
    /// Executes incremental state recovery: loads latest snapshot, restores state, and replays remaining log entries.
    ///
    /// Returns `(replayed_event_count, snapshot_restored)`.
    pub fn recover<S, E, T, C, L, SS>(
        snapshot_store: &SS,
        snapshot_codec: &C,
        log: &L,
        target: &mut T,
        batch_size: usize,
    ) -> Result<(usize, bool), EventPublishError>
    where
        T: ReplayTarget<E> + RestoreFromSnapshot<S>,
        C: SnapshotCodec<S>,
        L: EventLog<E>,
        SS: SnapshotStore,
    {
        let latest_snapshot_opt = snapshot_store.load_latest_snapshot()?;
        let (start_sequence, snapshot_restored) = match latest_snapshot_opt {
            Some(ref snapshot) => {
                if !snapshot.verify_checksum() {
                    return Err(EventPublishError::StorageError(
                        "Snapshot checksum verification failed during recovery".to_string(),
                    ));
                }
                let state_instance = snapshot_codec.decode(&snapshot.state_payload)?;
                target.restore_snapshot(&state_instance);
                (SequenceNumber(snapshot.snapshot_sequence.0 + 1), true)
            }
            None => (SequenceNumber(1), false),
        };

        let replayed_count =
            LogReplayEngine::replay_from_offset(log, target, start_sequence, batch_size)?;

        Ok((replayed_count, snapshot_restored))
    }
}
