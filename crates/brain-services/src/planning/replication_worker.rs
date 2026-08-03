//! Per-Follower `ReplicationWorker`, `ReplicationCoordinator`, and `ReplicationBatch` (Phase 13 Milestone 13.1).
//!
//! ### Architectural Invariants:
//! 1. Decoupled Coordinator vs Worker: `ReplicationCoordinator` manages worker lifecycles, wake-ups, and heartbeat scheduling; `ReplicationWorker` manages a single follower replication stream.
//! 2. Immutable `ReplicationBatch<E>` Artifact: Transport batches are immutable artifacts; retries create new batches rather than mutating existing ones.
//! 3. Explicit Telemetry `ReplicationWorkerState`: Worker states (`Idle`, `Fetching`, `Sending`, `WaitingAck`, `BackingOff`, `SnapshotRequired`) provide complete diagnostic observability.
//! 4. Transport-Neutral `ReplicationTask<E>`: Abstract work items support both `AppendEntries` data/heartbeat batches and reserved `InstallSnapshot` tasks.

use crate::planning::cluster::NodeId;
use crate::planning::consensus::{
    AppendEntriesRequest, AppendEntriesResponse, LogReplicationState, TermId,
};
use crate::planning::durable_event_store::{EventEnvelope, SequenceNumber};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// Runtime operational telemetry state of a `ReplicationWorker`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplicationWorkerState {
    /// Worker idle waiting for log entries or heartbeat interval.
    Idle,
    /// Worker fetching envelopes from event log.
    Fetching,
    /// Worker transmitting replication batch over transport.
    Sending,
    /// Worker awaiting response acknowledgement from follower.
    WaitingAck,
    /// Worker backing off due to network retry or failure.
    BackingOff,
    /// Follower log is truncated behind snapshot cutoff; snapshot required.
    SnapshotRequired,
}

/// Explicit classification of `ReplicationBatch` purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplicationBatchKind {
    /// Data batch containing domain event envelopes.
    Data,
    /// Empty heartbeat batch maintaining leader authority.
    Heartbeat,
}

/// Immutable replication payload batch artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationBatch<E> {
    /// Target follower node ID.
    pub target_node: NodeId,
    /// Sequence number offset of first entry in batch.
    pub start_sequence: SequenceNumber,
    /// Event envelopes included in batch.
    pub entries: Vec<EventEnvelope<E>>,
    /// Total encoded byte size of payload.
    pub batch_bytes: usize,
    /// Batch classification kind.
    pub kind: ReplicationBatchKind,
}

/// Transport-neutral replication work task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicationTask<E> {
    /// Standard log entry replication or heartbeat request.
    AppendEntries(AppendEntriesRequest<E>),
    /// Snapshot installation request (reserved for Milestone 13.2).
    InstallSnapshot,
}

/// Single-follower replication stream manager.
#[derive(Debug)]
pub struct ReplicationWorker {
    target_node: NodeId,
    state: ReplicationWorkerState,
    replication_state: LogReplicationState,
}

impl ReplicationWorker {
    /// Instantiates a new `ReplicationWorker` for a target follower node.
    pub fn new(target_node: NodeId, last_log_index: SequenceNumber) -> Self {
        Self {
            target_node,
            state: ReplicationWorkerState::Idle,
            replication_state: LogReplicationState::new(last_log_index),
        }
    }

    /// Returns target follower node ID.
    pub fn target_node(&self) -> NodeId {
        self.target_node
    }

    /// Returns current runtime worker telemetry state.
    pub fn state(&self) -> ReplicationWorkerState {
        self.state
    }

    /// Returns reference to follower `LogReplicationState`.
    pub fn replication_state(&self) -> LogReplicationState {
        self.replication_state
    }

    /// Transitions worker telemetry state.
    pub fn set_state(&mut self, state: ReplicationWorkerState) {
        self.state = state;
    }

    /// Constructs an immutable `ReplicationBatch` from fetched log envelopes.
    pub fn create_data_batch<E: Clone>(
        &self,
        start_sequence: SequenceNumber,
        envelopes: Vec<EventEnvelope<E>>,
    ) -> ReplicationBatch<E> {
        let kind = if envelopes.is_empty() {
            ReplicationBatchKind::Heartbeat
        } else {
            ReplicationBatchKind::Data
        };

        ReplicationBatch {
            target_node: self.target_node,
            start_sequence,
            entries: envelopes,
            batch_bytes: 0,
            kind,
        }
    }

    /// Constructs an `AppendEntriesRequest` task from a `ReplicationBatch`.
    pub fn build_append_entries_task<E: Clone>(
        &self,
        batch: &ReplicationBatch<E>,
        term: TermId,
        leader_id: NodeId,
        prev_log_index: SequenceNumber,
        prev_log_term: TermId,
        leader_commit: SequenceNumber,
    ) -> ReplicationTask<E> {
        ReplicationTask::AppendEntries(AppendEntriesRequest {
            term,
            leader_id,
            prev_log_index,
            prev_log_term,
            entries: batch.entries.clone(),
            leader_commit,
        })
    }

    /// Processes an `AppendEntriesResponse` from follower, updating `match_index` and `next_index`.
    pub fn process_response(&mut self, response: &AppendEntriesResponse) {
        if response.success {
            self.replication_state.match_index = response.match_index;
            self.replication_state.next_index = SequenceNumber(response.match_index.0 + 1);
            self.state = ReplicationWorkerState::Idle;
        } else {
            // Decrement next_index on log mismatch
            if self.replication_state.next_index.0 > 1 {
                self.replication_state.next_index =
                    SequenceNumber(self.replication_state.next_index.0 - 1);
            }
            self.state = ReplicationWorkerState::BackingOff;
        }
    }
}

/// Orchestrator managing multi-follower `ReplicationWorker` tasks.
#[derive(Debug, Default)]
pub struct ReplicationCoordinator {
    workers: Mutex<HashMap<NodeId, ReplicationWorker>>,
}

impl ReplicationCoordinator {
    /// Instantiates a new `ReplicationCoordinator`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new follower node into replication coordination.
    pub fn register_follower(&self, target_node: NodeId, last_log_index: SequenceNumber) {
        let mut guard = self.workers.lock().unwrap();
        guard.insert(
            target_node,
            ReplicationWorker::new(target_node, last_log_index),
        );
    }

    /// Deregisters a follower node upon departure.
    pub fn deregister_follower(&self, target_node: &NodeId) {
        let mut guard = self.workers.lock().unwrap();
        guard.remove(target_node);
    }

    /// Returns list of currently registered follower node IDs.
    pub fn registered_followers(&self) -> Vec<NodeId> {
        let guard = self.workers.lock().unwrap();
        guard.keys().copied().collect()
    }

    /// Executes a closure over a specific follower worker.
    pub fn with_worker<F, R>(&self, target_node: &NodeId, f: F) -> Option<R>
    where
        F: FnOnce(&mut ReplicationWorker) -> R,
    {
        let mut guard = self.workers.lock().unwrap();
        guard.get_mut(target_node).map(f)
    }
}
