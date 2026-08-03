//! Decoupled Snapshot Replication Planner, Immutable `SnapshotChunk` Artifacts & `SnapshotReplicator` (Phase 13 Milestone 13.2).
//!
//! ### Architectural Invariants:
//! 1. Planner vs Executor Separation: `SnapshotReplicationPlanner` compiles an immutable `SnapshotTransferPlan`; `SnapshotReplicator` streams chunks over the transport.
//! 2. Immutable `SnapshotChunk` Artifact: Chunks are immutable artifacts carrying a strongly-typed `SnapshotTransferId(pub Uuid)` for correlation.
//! 3. Strict Resumable Offset Invariant: `offset(N+1) == offset(N) + bytes_sent(N)`.
//! 4. Observable Transfer State: `SnapshotTransferState` (`Preparing`, `Streaming`, `WaitingAck`, `Completed`, `Failed`) provides explicit telemetry observability.

use crate::planning::cluster::NodeId;
use crate::planning::consensus::{InstallSnapshotRequest, TermId};
use crate::planning::durable_event_store::SequenceNumber;
use crate::planning::snapshot_store::LogSnapshot;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Strongly-typed identifier for correlated snapshot transfer sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SnapshotTransferId(pub Uuid);

impl std::fmt::Display for SnapshotTransferId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "transfer_{}", self.0)
    }
}

/// Explicit runtime state of a snapshot transfer session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SnapshotTransferState {
    /// Transfer session initialized; building plan.
    Preparing,
    /// Actively streaming snapshot chunks over transport.
    Streaming,
    /// Awaiting ACK response for transmitted chunk.
    WaitingAck,
    /// Transfer fully completed and verified.
    Completed,
    /// Transfer failed (checksum mismatch or disconnect).
    Failed,
}

/// Immutable snapshot chunk artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotChunk {
    /// Associated snapshot transfer session ID.
    pub transfer_id: SnapshotTransferId,
    /// 0-based chunk index in stream sequence.
    pub chunk_index: usize,
    /// Byte offset in target snapshot file (`offset(N+1) == offset(N) + bytes_sent(N)`).
    pub offset: u64,
    /// Raw payload bytes of chunk.
    pub data: Vec<u8>,
    /// `true` if this chunk is the final payload in the stream.
    pub is_last: bool,
}

/// Immutable snapshot transfer plan compiled by `SnapshotReplicationPlanner`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotTransferPlan {
    /// Unique transfer session ID.
    pub transfer_id: SnapshotTransferId,
    /// Target follower node ID.
    pub target_node: NodeId,
    /// Source snapshot ID.
    pub snapshot_id: Uuid,
    /// Total byte size of complete snapshot.
    pub total_bytes: usize,
    /// Maximum byte size per chunk.
    pub chunk_size: usize,
    /// Total number of chunks in plan.
    pub total_chunks: usize,
    /// Ordered list of immutable `SnapshotChunk` artifacts.
    pub chunks: Vec<SnapshotChunk>,
}

/// Pure planner compiling `SnapshotTransferPlan` artifacts.
pub struct SnapshotReplicationPlanner;

impl SnapshotReplicationPlanner {
    /// Compiles an immutable `SnapshotTransferPlan` from a `LogSnapshot`.
    pub fn plan_transfer(
        snapshot: &LogSnapshot,
        target_node: NodeId,
        chunk_size: usize,
    ) -> SnapshotTransferPlan {
        let transfer_id = SnapshotTransferId(Uuid::new_v4());
        let effective_chunk_size = if chunk_size == 0 { 65536 } else { chunk_size };
        let payload_bytes = &snapshot.state_payload;
        let total_bytes = payload_bytes.len();

        let mut chunks = Vec::new();
        if total_bytes == 0 {
            chunks.push(SnapshotChunk {
                transfer_id,
                chunk_index: 0,
                offset: 0,
                data: vec![],
                is_last: true,
            });
        } else {
            let mut offset = 0usize;
            let mut chunk_index = 0usize;

            while offset < total_bytes {
                let end = (offset + effective_chunk_size).min(total_bytes);
                let is_last = end == total_bytes;
                let chunk_data = payload_bytes[offset..end].to_vec();

                chunks.push(SnapshotChunk {
                    transfer_id,
                    chunk_index,
                    offset: offset as u64,
                    data: chunk_data,
                    is_last,
                });

                offset = end;
                chunk_index += 1;
            }
        }

        let total_chunks = chunks.len();

        SnapshotTransferPlan {
            transfer_id,
            target_node,
            snapshot_id: snapshot.snapshot_id,
            total_bytes,
            chunk_size: effective_chunk_size,
            total_chunks,
            chunks,
        }
    }
}

/// Executing manager driving snapshot transfer plans over transport.
#[derive(Debug)]
pub struct SnapshotReplicator {
    plan: SnapshotTransferPlan,
    current_chunk_idx: usize,
    state: SnapshotTransferState,
}

impl SnapshotReplicator {
    /// Instantiates a new `SnapshotReplicator` for a target transfer plan.
    pub fn new(plan: SnapshotTransferPlan) -> Self {
        Self {
            plan,
            current_chunk_idx: 0,
            state: SnapshotTransferState::Preparing,
        }
    }

    /// Returns reference to current transfer plan.
    pub fn plan(&self) -> &SnapshotTransferPlan {
        &self.plan
    }

    /// Returns current runtime transfer state.
    pub fn state(&self) -> SnapshotTransferState {
        self.state
    }

    /// Fetches next `InstallSnapshotRequest` to transmit over transport.
    pub fn next_request(
        &mut self,
        term: TermId,
        leader_id: NodeId,
        last_included_sequence: SequenceNumber,
        last_included_term: TermId,
    ) -> Option<InstallSnapshotRequest> {
        if self.current_chunk_idx >= self.plan.chunks.len() {
            self.state = SnapshotTransferState::Completed;
            return None;
        }

        let chunk = &self.plan.chunks[self.current_chunk_idx];
        self.state = SnapshotTransferState::Streaming;

        Some(InstallSnapshotRequest {
            term,
            leader_id,
            last_included_sequence,
            last_included_term,
            offset: chunk.offset,
            data: chunk.data.clone(),
            done: chunk.is_last,
        })
    }

    /// Processes follower acknowledgement for sent chunk.
    pub fn process_ack(&mut self, success: bool, bytes_written: u64) {
        if success {
            let expected_offset = self.plan.chunks[self.current_chunk_idx].offset
                + self.plan.chunks[self.current_chunk_idx].data.len() as u64;

            if bytes_written == expected_offset {
                self.current_chunk_idx += 1;
                if self.current_chunk_idx >= self.plan.chunks.len() {
                    self.state = SnapshotTransferState::Completed;
                } else {
                    self.state = SnapshotTransferState::Streaming;
                }
            } else {
                self.state = SnapshotTransferState::Failed;
            }
        } else {
            self.state = SnapshotTransferState::Failed;
        }
    }
}
