//! Event-Sourced `ReplicationProjection` & Pure `ReplicationHealthEvaluator` (Phase 14 Milestone 14.1).
//!
//! ### Architectural Invariants:
//! 1. Separated Metrics vs Derived Health: `FollowerReplicationMetrics` tracks pure operational measurements; `ReplicationHealthEvaluator` pure function derives `ReplicationHealth`.
//! 2. Lazy Time-Windowed Throughput: Bytes/sec and envelopes/sec are computed lazily from `first_seen_ms` and `last_seen_ms` to preserve replay determinism.
//! 3. Replay Target Trait Implementation: `ReplicationProjection` implements `ReplayTarget<ReplicationEvent>` for deterministic event replay.

use crate::planning::cluster::NodeId;
use crate::planning::durable_event_store::{EventEnvelope, SequenceNumber};
use crate::planning::log_replay_engine::ReplayTarget;
use crate::planning::replication_events::{ReplicationEvent, ReplicationEventKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Health classification for follower replication streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplicationHealth {
    /// Follower is up to date within acceptable lag bounds.
    Healthy,
    /// Follower is falling behind leader sequence index.
    Lagging,
    /// Follower worker is in backoff state due to retries or failures.
    Backoff,
    /// Follower log truncated behind snapshot cutoff; snapshot required.
    SnapshotRequired,
    /// Follower worker is offline or deregistered.
    Offline,
}

/// Operational telemetry metrics tracked per follower node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowerReplicationMetrics {
    /// Target node ID.
    pub target_node: NodeId,
    /// Highest matched sequence index acknowledged by follower.
    pub match_index: SequenceNumber,
    /// Calculated sequence index lag relative to leader.
    pub lag_entries: u64,
    /// Total byte payload transmitted to follower.
    pub bytes_sent: u64,
    /// Total ACK responses received from follower.
    pub ack_count: u64,
    /// Total consecutive failure retries.
    pub retry_count: u32,
    /// Timestamp in milliseconds when worker was first seen.
    pub first_seen_ms: u64,
    /// Timestamp in milliseconds of last received event.
    pub last_seen_ms: u64,
    /// Is worker currently active and registered.
    pub is_active: bool,
    /// Is snapshot currently required for follower catchup.
    pub snapshot_required: bool,
}

impl FollowerReplicationMetrics {
    /// Instantiates default metrics for a node.
    pub fn new(target_node: NodeId, timestamp_ms: u64, initial_next_index: SequenceNumber) -> Self {
        let match_idx = SequenceNumber(initial_next_index.0.saturating_sub(1));
        Self {
            target_node,
            match_index: match_idx,
            lag_entries: 0,
            bytes_sent: 0,
            ack_count: 0,
            retry_count: 0,
            first_seen_ms: timestamp_ms,
            last_seen_ms: timestamp_ms,
            is_active: true,
            snapshot_required: false,
        }
    }

    /// Computes lazy time-windowed bytes-per-second throughput.
    pub fn bytes_per_second(&self) -> f64 {
        let elapsed_sec = (self.last_seen_ms.saturating_sub(self.first_seen_ms)) as f64 / 1000.0;
        if elapsed_sec <= 0.0 {
            0.0
        } else {
            self.bytes_sent as f64 / elapsed_sec
        }
    }
}

/// Pure policy evaluator deriving `ReplicationHealth` from operational metrics.
pub struct ReplicationHealthEvaluator;

impl ReplicationHealthEvaluator {
    /// Evaluates `ReplicationHealth` deterministically from metrics and current leader sequence.
    pub fn evaluate_health(
        metrics: &FollowerReplicationMetrics,
        current_leader_seq: SequenceNumber,
    ) -> ReplicationHealth {
        if !metrics.is_active {
            return ReplicationHealth::Offline;
        }

        if metrics.snapshot_required {
            return ReplicationHealth::SnapshotRequired;
        }

        if metrics.retry_count > 0 {
            return ReplicationHealth::Backoff;
        }

        let lag = current_leader_seq.0.saturating_sub(metrics.match_index.0);
        if lag > 50 {
            ReplicationHealth::Lagging
        } else {
            ReplicationHealth::Healthy
        }
    }
}

/// Event-sourced in-memory projection tracking replication metrics across followers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationProjection {
    /// Followers mapped by node ID.
    pub followers: HashMap<NodeId, FollowerReplicationMetrics>,
    /// Highest sequence number processed by projection.
    pub last_sequence: SequenceNumber,
}

impl Default for ReplicationProjection {
    fn default() -> Self {
        Self {
            followers: HashMap::new(),
            last_sequence: SequenceNumber(0),
        }
    }
}

impl ReplicationProjection {
    /// Instantiates a new `ReplicationProjection`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Accesses follower metrics for a target node.
    pub fn get_metrics(&self, node_id: &NodeId) -> Option<&FollowerReplicationMetrics> {
        self.followers.get(node_id)
    }

    /// Evaluates derived health for a target node.
    pub fn get_health(
        &self,
        node_id: &NodeId,
        current_leader_seq: SequenceNumber,
    ) -> ReplicationHealth {
        match self.followers.get(node_id) {
            Some(metrics) => {
                ReplicationHealthEvaluator::evaluate_health(metrics, current_leader_seq)
            }
            None => ReplicationHealth::Offline,
        }
    }

    /// Returns last sequence index applied to projection.
    pub fn last_applied_sequence(&self) -> SequenceNumber {
        self.last_sequence
    }
}

impl ReplayTarget<ReplicationEvent> for ReplicationProjection {
    fn apply_envelope(&mut self, env: &EventEnvelope<ReplicationEvent>) {
        if env.sequence.0 <= self.last_sequence.0 && self.last_sequence.0 > 0 {
            return;
        }
        self.last_sequence = env.sequence;
        let evt = &env.payload;
        let node_id = evt.target_node;

        let metrics = self.followers.entry(node_id).or_insert_with(|| {
            FollowerReplicationMetrics::new(node_id, evt.timestamp_ms, SequenceNumber(1))
        });

        metrics.last_seen_ms = evt.timestamp_ms;

        match &evt.kind {
            ReplicationEventKind::WorkerRegistered { initial_next_index } => {
                metrics.is_active = true;
                metrics.match_index = SequenceNumber(initial_next_index.0.saturating_sub(1));
                metrics.retry_count = 0;
                metrics.snapshot_required = false;
            }
            ReplicationEventKind::BatchSent { bytes_count, .. } => {
                metrics.bytes_sent += *bytes_count as u64;
            }
            ReplicationEventKind::AckReceived { match_index, .. } => {
                metrics.match_index = *match_index;
                metrics.ack_count += 1;
                metrics.retry_count = 0;
                metrics.snapshot_required = false;
            }
            ReplicationEventKind::RetryScheduled {
                consecutive_failures,
                ..
            } => {
                metrics.retry_count = *consecutive_failures;
            }
            ReplicationEventKind::SnapshotRequested { .. } => {
                metrics.snapshot_required = true;
            }
            ReplicationEventKind::WorkerDeregistered => {
                metrics.is_active = false;
            }
            ReplicationEventKind::ReplicationRecovered => {
                metrics.retry_count = 0;
                metrics.snapshot_required = false;
            }
            _ => {}
        }
    }
}
