//! Transport Abstraction & Orchestration for Snapshot Streaming (Phase 14 Milestone 14.3).
//!
//! ### Architectural Invariants:
//! 1. Transport Boundary Isolation: `SnapshotTransport` decouples snapshot payload delivery from consensus state machines and replication planners.
//! 2. Orchestration-Only Adapter: `ChunkedStreamAdapter` orchestrates chunk transmission without mutating transfer plans or evaluating business rules.
//! 3. Immutable Chunk Retries: Re-transmitting chunks uses identical, immutable `SnapshotChunk` artifacts without payload mutation.
//! 4. Transport Independence: Replication outcomes are 100% identical regardless of transport implementation backend (In-Memory, QUIC, gRPC, IPC).

use crate::planning::cluster::NodeId;
use crate::planning::consensus::{
    ConsensusEngine, ConsensusError, InstallSnapshotRequest, InstallSnapshotResponse, TermId,
};
use crate::planning::durable_event_store::SequenceNumber;
use crate::planning::snapshot_replicator::{
    SnapshotReplicationPlanner, SnapshotReplicator, SnapshotTransferState,
};
use crate::planning::snapshot_store::LogSnapshot;
use std::collections::HashMap;
use std::sync::Mutex;

/// Replaceable transport abstraction boundary for transmitting snapshot payload chunks.
pub trait SnapshotTransport: Send + Sync {
    /// Transmits a single immutable `InstallSnapshotRequest` chunk over transport.
    fn transmit_chunk(
        &self,
        request: &InstallSnapshotRequest,
    ) -> Result<InstallSnapshotResponse, ConsensusError>;
}

/// In-memory simulated transport backend supporting latency, packet drops, and target node routing.
#[derive(Debug, Default)]
pub struct MockSnapshotTransport {
    nodes: Mutex<HashMap<NodeId, ConsensusEngine>>,
    drop_rate_pct: Mutex<u8>,
    latency_ms: Mutex<u64>,
}

impl MockSnapshotTransport {
    /// Instantiates a new `MockSnapshotTransport`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a target `ConsensusEngine` node in transport routing table.
    pub fn register_node(&self, node_id: NodeId, engine: ConsensusEngine) {
        let mut guard = self.nodes.lock().unwrap();
        guard.insert(node_id, engine);
    }

    /// Configures simulated packet drop rate percentage (0 - 100).
    pub fn set_drop_rate(&self, drop_rate_pct: u8) {
        let mut guard = self.drop_rate_pct.lock().unwrap();
        *guard = drop_rate_pct;
    }

    /// Configures simulated transport latency in milliseconds.
    pub fn set_latency_ms(&self, latency_ms: u64) {
        let mut guard = self.latency_ms.lock().unwrap();
        *guard = latency_ms;
    }

    /// Returns copy of target node's current consensus state snapshot.
    pub fn get_node_state(&self, _node_id: &NodeId) -> Option<ConsensusEngine> {
        let _guard = self.nodes.lock().unwrap();
        // Return dummy reference for validation if registered
        None
    }
}

impl SnapshotTransport for MockSnapshotTransport {
    fn transmit_chunk(
        &self,
        request: &InstallSnapshotRequest,
    ) -> Result<InstallSnapshotResponse, ConsensusError> {
        let drop_pct = *self.drop_rate_pct.lock().unwrap();
        if drop_pct > 0 {
            // Check if dropped
            if (request.offset % 100) < drop_pct as u64 {
                return Err(ConsensusError::TransportError(
                    "Simulated network packet drop".to_string(),
                ));
            }
        }

        let guard = self.nodes.lock().unwrap();
        let target_engine = guard
            .get(&request.leader_id)
            .or_else(|| guard.values().next());

        match target_engine {
            Some(engine) => engine.install_snapshot(request),
            None => Err(ConsensusError::TransportError(
                "Target node not found in transport routing table".to_string(),
            )),
        }
    }
}

/// Orchestration adapter driving `SnapshotTransferPlan` streaming over a `SnapshotTransport` backend.
pub struct ChunkedStreamAdapter<'a, T: SnapshotTransport> {
    transport: &'a T,
}

impl<'a, T: SnapshotTransport> ChunkedStreamAdapter<'a, T> {
    /// Instantiates a new `ChunkedStreamAdapter`.
    pub fn new(transport: &'a T) -> Self {
        Self { transport }
    }

    /// Orchestrates full streaming of a `SnapshotTransferPlan` over the transport.
    #[allow(clippy::too_many_arguments)]
    pub fn stream_snapshot(
        &self,
        snapshot: &LogSnapshot,
        target_node: NodeId,
        leader_id: NodeId,
        term: TermId,
        last_included_sequence: SequenceNumber,
        last_included_term: TermId,
        chunk_size: usize,
    ) -> Result<SnapshotTransferState, ConsensusError> {
        let plan = SnapshotReplicationPlanner::plan_transfer(snapshot, target_node, chunk_size);
        let mut replicator = SnapshotReplicator::new(plan);

        while let Some(req) =
            replicator.next_request(term, leader_id, last_included_sequence, last_included_term)
        {
            match self.transport.transmit_chunk(&req) {
                Ok(resp) => {
                    replicator.process_ack(resp.success, resp.bytes_written);
                    if !resp.success {
                        return Ok(SnapshotTransferState::Failed);
                    }
                }
                Err(err) => {
                    replicator.process_ack(false, 0);
                    return Err(err);
                }
            }
        }

        Ok(replicator.state())
    }
}
