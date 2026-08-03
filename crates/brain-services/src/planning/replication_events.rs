//! Event-Sourced Domain Events & Publisher for Cluster Replication (Phase 14 Milestone 14.1).
//!
//! ### Architectural Invariants:
//! 1. Decoupled Publisher Boundary: `ReplicationEventPublisher` isolates event generation/publishing from `ReplicationWorker` transport execution.
//! 2. Immutable Event Artifacts: `ReplicationEvent` envelopes carry strongly-typed `ReplicationEventId(pub Uuid)` and schema version headers.
//! 3. Causal Ordering Invariant: Events follow deterministic ordering (`WorkerRegistered` -> `BatchSent` -> `AckReceived` -> `WorkerDeregistered`).

use crate::planning::cluster::NodeId;
use crate::planning::durable_event_store::{EventEnvelope, SequenceNumber};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Constant schema version header for `ReplicationEvent`.
pub const REPLICATION_EVENT_SCHEMA_VERSION: u16 = 1;

/// Strongly-typed identifier for replication events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReplicationEventId(pub Uuid);

impl std::fmt::Display for ReplicationEventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "repl_evt_{}", self.0)
    }
}

/// Domain event classification variants for replication stream observability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicationEventKind {
    /// Follower replication worker registered in coordinator.
    WorkerRegistered {
        /// Initial sequence next index for follower.
        initial_next_index: SequenceNumber,
    },
    /// Replication batch transmitted over transport.
    BatchSent {
        /// Sequence number offset of first entry in batch.
        start_sequence: SequenceNumber,
        /// Number of event envelopes included in batch.
        entry_count: usize,
        /// Total encoded byte size of batch.
        bytes_count: usize,
    },
    /// Acknowledgement received from follower.
    AckReceived {
        /// Highest sequence index matched on follower.
        match_index: SequenceNumber,
        /// Round-trip time in milliseconds.
        rtt_ms: u64,
    },
    /// Replication retry scheduled due to failure or mismatch.
    RetryScheduled {
        /// Total consecutive failure attempts.
        consecutive_failures: u32,
        /// Penalty backoff delay in milliseconds.
        backoff_ms: u64,
    },
    /// Follower log truncated behind snapshot cutoff; snapshot requested.
    SnapshotRequested {
        /// Snapshot sequence cutoff index.
        snapshot_sequence: SequenceNumber,
    },
    /// Follower replication worker deregistered.
    WorkerDeregistered,
    /// Replication stream paused.
    ReplicationPaused,
    /// Replication stream resumed.
    ReplicationResumed,
    /// Replication stream recovered back to healthy state.
    ReplicationRecovered,
    /// Snapshot transfer completed successfully.
    SnapshotCompleted,
    /// Flow control limits dynamically adjusted.
    FlowControlAdjusted {
        /// Recommended maximum batch size.
        batch_size: usize,
        /// Pacing delay in milliseconds.
        pacing_ms: u64,
    },
}

/// Immutable event-sourced domain event artifact for cluster replication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationEvent {
    /// Schema version header.
    pub schema_version: u16,
    /// Unique event identifier.
    pub event_id: ReplicationEventId,
    /// Timestamp in milliseconds when event occurred.
    pub timestamp_ms: u64,
    /// Target follower node ID.
    pub target_node: NodeId,
    /// Event classification kind.
    pub kind: ReplicationEventKind,
}

impl ReplicationEvent {
    /// Instantiates a new `ReplicationEvent`.
    pub fn new(timestamp_ms: u64, target_node: NodeId, kind: ReplicationEventKind) -> Self {
        Self {
            schema_version: REPLICATION_EVENT_SCHEMA_VERSION,
            event_id: ReplicationEventId(Uuid::new_v4()),
            timestamp_ms,
            target_node,
            kind,
        }
    }
}

/// Publisher interface isolating replication event emission from worker execution.
pub struct ReplicationEventPublisher;

impl ReplicationEventPublisher {
    /// Wraps a `ReplicationEvent` payload into a sequence-allocated `EventEnvelope`.
    pub fn create_envelope(
        sequence: SequenceNumber,
        event: ReplicationEvent,
    ) -> EventEnvelope<ReplicationEvent> {
        EventEnvelope {
            sequence,
            timestamp_ms: event.timestamp_ms,
            schema_version: event.schema_version,
            payload: event,
        }
    }
}
