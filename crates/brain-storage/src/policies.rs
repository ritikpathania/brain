//! Lifecycle policies for Snapshot, Retention, Compaction, and Storage Lifecycle Orchestration.

use crate::errors::StorageError;
use crate::traits::SnapshotStore;
use brain_events::EventStore;
use serde::{Deserialize, Serialize};

/// Current snapshot format schema version.
pub const CURRENT_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Explicit versioned metadata header for binary snapshot artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotHeader {
    /// Schema format version (CURRENT_SNAPSHOT_SCHEMA_VERSION = 1).
    pub schema_version: u32,
    /// Incremental snapshot version number.
    pub snapshot_version: u32,
    /// Creation timestamp in milliseconds.
    pub created_at_ms: u64,
    /// Last included event sequence number.
    pub event_sequence: u64,
    /// CRC32 integrity checksum of payload.
    pub checksum: u32,
}

impl SnapshotHeader {
    /// Instantiates a new `SnapshotHeader`.
    pub fn new(
        snapshot_version: u32,
        created_at_ms: u64,
        event_sequence: u64,
        checksum: u32,
    ) -> Self {
        Self {
            schema_version: CURRENT_SNAPSHOT_SCHEMA_VERSION,
            snapshot_version,
            created_at_ms,
            event_sequence,
            checksum,
        }
    }
}

/// Policy controlling snapshot triggering based on event count and duration thresholds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPolicy {
    /// Maximum event count between snapshot triggers.
    pub max_events: usize,
    /// Maximum duration in milliseconds between snapshot triggers.
    pub max_duration_ms: u64,
}

impl SnapshotPolicy {
    /// Instantiates a new `SnapshotPolicy`.
    pub fn new(max_events: usize, max_duration_ms: u64) -> Self {
        Self {
            max_events,
            max_duration_ms,
        }
    }

    /// Evaluates if a new snapshot should be triggered based on events count and elapsed duration.
    pub fn should_snapshot(&self, events_since_snapshot: usize, elapsed_ms: u64) -> bool {
        events_since_snapshot >= self.max_events || elapsed_ms >= self.max_duration_ms
    }
}

/// Policy controlling retention TTL cutoffs for event logs and snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Time-to-live retention duration in milliseconds.
    pub retention_ttl_ms: u64,
}

impl RetentionPolicy {
    /// Instantiates a new `RetentionPolicy`.
    pub fn new(retention_ttl_ms: u64) -> Self {
        Self { retention_ttl_ms }
    }

    /// Calculates the cutoff timestamp prior to which events can be safely pruned.
    pub fn calculate_cutoff_timestamp(&self, current_timestamp_ms: u64) -> u64 {
        current_timestamp_ms.saturating_sub(self.retention_ttl_ms)
    }
}

/// Policy controlling event log compaction thresholds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionPolicy {
    /// Minimum count of old events required before executing log compaction.
    pub compaction_threshold: usize,
}

impl CompactionPolicy {
    /// Instantiates a new `CompactionPolicy`.
    pub fn new(compaction_threshold: usize) -> Self {
        Self {
            compaction_threshold,
        }
    }

    /// Evaluates if compaction should run based on total log length.
    pub fn should_compact(&self, total_event_count: usize) -> bool {
        total_event_count >= self.compaction_threshold
    }
}

/// Lifecycle orchestrator driving snapshots and retention compaction across storage backends.
pub struct StorageLifecycleOrchestrator {
    /// Policy governing snapshot creation triggers.
    pub snapshot_policy: SnapshotPolicy,
    /// Policy governing retention TTL cutoffs.
    pub retention_policy: RetentionPolicy,
    /// Policy governing log compaction thresholds.
    pub compaction_policy: CompactionPolicy,
}

impl StorageLifecycleOrchestrator {
    /// Instantiates a new `StorageLifecycleOrchestrator`.
    pub fn new(
        snapshot_policy: SnapshotPolicy,
        retention_policy: RetentionPolicy,
        compaction_policy: CompactionPolicy,
    ) -> Self {
        Self {
            snapshot_policy,
            retention_policy,
            compaction_policy,
        }
    }

    /// Executes snapshot creation using a provided binary state slice and versioned header.
    pub fn execute_snapshot(
        &self,
        snapshot_store: &dyn SnapshotStore,
        snapshot_id: &str,
        header: &SnapshotHeader,
        state_bytes: &[u8],
    ) -> Result<(), StorageError> {
        let header_json =
            serde_json::to_vec(header).map_err(|e| StorageError::Serialization(e.to_string()))?;

        let mut blob = Vec::new();
        let header_len = header_json.len() as u32;
        blob.extend_from_slice(&header_len.to_le_bytes());
        blob.extend_from_slice(&header_json);
        blob.extend_from_slice(state_bytes);

        snapshot_store.save_snapshot(snapshot_id, &blob)
    }

    /// Restores snapshot payload and verifies versioned header.
    pub fn restore_snapshot(
        &self,
        snapshot_store: &dyn SnapshotStore,
        snapshot_id: &str,
    ) -> Result<Option<(SnapshotHeader, Vec<u8>)>, StorageError> {
        let blob = match snapshot_store.load_snapshot(snapshot_id)? {
            Some(b) => b,
            None => return Ok(None),
        };

        if blob.len() < 4 {
            return Err(StorageError::Serialization(
                "Corrupt snapshot blob: missing header length".to_string(),
            ));
        }

        let header_len = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
        if blob.len() < 4 + header_len {
            return Err(StorageError::Serialization(
                "Corrupt snapshot blob: truncated header".to_string(),
            ));
        }

        let header_bytes = &blob[4..4 + header_len];
        let state_bytes = &blob[4 + header_len..];

        let header: SnapshotHeader = serde_json::from_slice(header_bytes)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        Ok(Some((header, state_bytes.to_vec())))
    }

    /// Executes retention compaction on an `EventStore` using the current timestamp.
    pub fn execute_compaction(
        &self,
        event_store: &dyn EventStore,
        current_timestamp_ms: u64,
    ) -> usize {
        let cutoff = self
            .retention_policy
            .calculate_cutoff_timestamp(current_timestamp_ms);
        event_store.compact(cutoff)
    }
}
