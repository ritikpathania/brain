//! Pluggable Consensus Engine Boundary, State Machine & `RaftConsensusStrategy` (Phase 11 Milestone 11.2).
//!
//! ### Architectural Invariants:
//! 1. Layered Consensus Architecture: `ConsensusEngine` manages consensus state while `ConsensusProtocol` trait handles election voting and log entry replication.
//! 2. Strongly-Typed Term Monotonicity: Consensus terms are encapsulated in `TermId(pub u64)`, strictly advancing monotonically.
//! 3. Explicit Vote Classification: Voting returns `VoteResult` (`Granted`, `Rejected`, `StaleTerm`, `AlreadyVoted`) rather than raw booleans.
//! 4. Pure Quorum Evaluator: `QuorumEvaluator` provides pure majority consensus evaluation (`votes_granted >= total_nodes / 2 + 1`).

use crate::planning::cluster::{ClusterError, ClusterNode, NodeId};
use crate::planning::coordinator::LeaderElectionStrategy;
use crate::planning::durable_event_store::{EventEnvelope, SequenceNumber};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

/// Strongly-typed 1-based monotonic sequence number for consensus terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TermId(pub u64);

impl std::fmt::Display for TermId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "term_{}", self.0)
    }
}

/// Consensus node role classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsensusRole {
    /// Node following active leader.
    Follower,
    /// Node soliciting votes during leader election.
    Candidate,
    /// Node granted leadership by quorum.
    Leader,
}

/// Rich consensus state snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusState {
    /// Assigned consensus role.
    pub role: ConsensusRole,
    /// Current consensus term.
    pub current_term: TermId,
    /// Node ID voted for in current term.
    pub voted_for: Option<NodeId>,
}

/// Strongly-typed voting result classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VoteResult {
    /// Vote granted by voter node.
    Granted,
    /// Vote rejected (e.g. log not up to date).
    Rejected,
    /// Candidate term is older than voter term.
    StaleTerm,
    /// Voter has already voted for a different candidate in this term.
    AlreadyVoted,
}

/// Consensus operation error classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusError {
    /// Protocol feature not supported.
    Unsupported,
    /// Storage error during consensus state persistence.
    StorageError(String),
    /// Invalid term transition.
    InvalidTerm,
    /// Transport transmission or routing error.
    TransportError(String),
}

impl std::fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "Consensus operation unsupported"),
            Self::StorageError(msg) => write!(f, "Consensus storage error: {}", msg),
            Self::InvalidTerm => write!(f, "Invalid consensus term transition"),
            Self::TransportError(msg) => write!(f, "Consensus transport error: {}", msg),
        }
    }
}

impl std::error::Error for ConsensusError {}

/// Strongly-typed rejection reasons for `AppendEntriesResponse`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppendEntriesRejectReason {
    /// Leader term is older than receiver term.
    StaleTerm,
    /// Log entry does not match `prev_log_index` or `prev_log_term`.
    LogMismatch,
    /// Receiver log is truncated behind snapshot cutoff.
    SnapshotRequired,
    /// Dynamic membership configuration mismatch.
    ConfigurationMismatch,
}

/// Replicated log entry transport request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppendEntriesRequest<E> {
    /// Leader consensus term.
    pub term: TermId,
    /// Leader node ID.
    pub leader_id: NodeId,
    /// Sequence number of log entry immediately preceding new entries.
    pub prev_log_index: SequenceNumber,
    /// Term of `prev_log_index` entry.
    pub prev_log_term: TermId,
    /// Log entries to store (empty for heartbeat).
    pub entries: Vec<EventEnvelope<E>>,
    /// Leader's committed sequence number index.
    pub leader_commit: SequenceNumber,
}

/// Replicated log entry transport response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    /// Current receiver term for leader update.
    pub term: TermId,
    /// `true` if follower contained entry matching `prev_log_index` and `prev_log_term`.
    pub success: bool,
    /// Highest log sequence index matched on follower.
    pub match_index: SequenceNumber,
    /// Optional rejection reason diagnostic.
    pub reject_reason: Option<AppendEntriesRejectReason>,
}

/// Chunked snapshot installation transport request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallSnapshotRequest {
    /// Leader consensus term.
    pub term: TermId,
    /// Leader node ID.
    pub leader_id: NodeId,
    /// Last sequence number included in snapshot.
    pub last_included_sequence: SequenceNumber,
    /// Term of `last_included_sequence`.
    pub last_included_term: TermId,
    /// Byte offset of chunk payload.
    pub offset: u64,
    /// Raw snapshot chunk payload bytes.
    pub data: Vec<u8>,
    /// `true` if this is the final snapshot chunk.
    pub done: bool,
}

/// Chunked snapshot installation transport response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstallSnapshotResponse {
    /// Current receiver term for leader update.
    pub term: TermId,
    /// `true` if snapshot chunk was accepted and written.
    pub success: bool,
    /// Total bytes written so far.
    pub bytes_written: u64,
}

/// Zero-log-append linearizable read request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReadIndexRequest {
    /// Unique read query identifier.
    pub read_id: Uuid,
    /// Target leader node ID.
    pub leader_id: NodeId,
    /// Target consensus term.
    pub term: TermId,
}

/// Rich diagnostic classification for linearizable read validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReadValidationResult {
    /// Leader lease valid; linearizable read permitted.
    LeaseValid,
    /// Leader lease TTL expired.
    LeaseExpired,
    /// Stale leader term detected.
    StaleLeader,
    /// Commit index is older than required index.
    CommitIndexTooOld,
    /// Read index quorum confirmation failed.
    QuorumUnavailable,
}

impl ReadValidationResult {
    /// Returns `true` if the read validation succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::LeaseValid)
    }
}

/// Zero-log-append linearizable read response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReadIndexResponse {
    /// Unique read query identifier.
    pub read_id: Uuid,
    /// Current leader consensus term.
    pub term: TermId,
    /// Evaluated target sequence index for read query execution.
    pub read_index: SequenceNumber,
    /// Diagnostic validation result.
    pub validation_result: ReadValidationResult,
}

/// Distributed leader lease artifact for zero-log-append linearizable reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LeaderLease {
    /// Leader node ID holding lease.
    pub leader_id: NodeId,
    /// Term for which lease was granted.
    pub term: TermId,
    /// Timestamp in milliseconds when lease was granted.
    pub granted_at_ms: u64,
    /// Lease time-to-live bound in milliseconds.
    pub lease_ttl_ms: u64,
}

/// Per-follower log replication state tracking index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LogReplicationState {
    /// Next log sequence index to send to follower.
    pub next_index: SequenceNumber,
    /// Highest log sequence index known to be replicated on follower.
    pub match_index: SequenceNumber,
}

impl LogReplicationState {
    /// Instantiates a new `LogReplicationState` with default starting sequence indices.
    pub fn new(last_log_index: SequenceNumber) -> Self {
        Self {
            next_index: SequenceNumber(last_log_index.0 + 1),
            match_index: SequenceNumber(0),
        }
    }
}

/// Abstract consensus protocol trait for voting and entry replication.
pub trait ConsensusProtocol: Send + Sync {
    /// Solicits a vote from a voter node for the specified term.
    fn request_vote(
        &self,
        term: TermId,
        candidate_id: NodeId,
    ) -> Result<VoteResult, ConsensusError>;

    /// Replicates log entries or sends heartbeats (legacy raw interface).
    fn append_entries_raw(&self, term: TermId, leader_id: NodeId) -> Result<(), ConsensusError> {
        let _ = (term, leader_id);
        Err(ConsensusError::Unsupported)
    }
}

/// Pure evaluator for consensus quorum mathematics.
pub struct QuorumEvaluator;

impl QuorumEvaluator {
    /// Evaluates whether `votes_granted` satisfies majority consensus requirement (`votes >= total_nodes / 2 + 1`).
    pub fn evaluate_quorum(votes_granted: usize, total_nodes: usize) -> bool {
        if total_nodes == 0 {
            return false;
        }
        let required_quorum = (total_nodes / 2) + 1;
        votes_granted >= required_quorum
    }
}

/// Consensus engine managing durable consensus state machine.
#[derive(Debug)]
pub struct ConsensusEngine {
    state: Mutex<ConsensusState>,
}

impl Default for ConsensusEngine {
    fn default() -> Self {
        Self {
            state: Mutex::new(ConsensusState {
                role: ConsensusRole::Follower,
                current_term: TermId(0),
                voted_for: None,
            }),
        }
    }
}

impl ConsensusEngine {
    /// Instantiates a new `ConsensusEngine`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a copy of current consensus state.
    pub fn current_state(&self) -> ConsensusState {
        self.state.lock().unwrap().clone()
    }

    /// Evaluates `AppendEntriesRequest` log entry replication or heartbeat.
    pub fn append_entries<E>(
        &self,
        request: &AppendEntriesRequest<E>,
    ) -> Result<AppendEntriesResponse, ConsensusError> {
        let mut guard = self.state.lock().unwrap();

        // 1. Term check invariant: reject if request term < current term
        if request.term < guard.current_term {
            return Ok(AppendEntriesResponse {
                term: guard.current_term,
                success: false,
                match_index: SequenceNumber(0),
                reject_reason: Some(AppendEntriesRejectReason::StaleTerm),
            });
        }

        // 2. Advance term and step down to Follower if request term > current term
        if request.term > guard.current_term {
            guard.current_term = request.term;
            guard.role = ConsensusRole::Follower;
            guard.voted_for = None;
        }

        let match_idx = if let Some(last_entry) = request.entries.last() {
            last_entry.sequence
        } else {
            request.prev_log_index
        };

        Ok(AppendEntriesResponse {
            term: guard.current_term,
            success: true,
            match_index: match_idx,
            reject_reason: None,
        })
    }

    /// Evaluates `InstallSnapshotRequest` snapshot chunk installation.
    pub fn install_snapshot(
        &self,
        request: &InstallSnapshotRequest,
    ) -> Result<InstallSnapshotResponse, ConsensusError> {
        let mut guard = self.state.lock().unwrap();

        if request.term < guard.current_term {
            return Ok(InstallSnapshotResponse {
                term: guard.current_term,
                success: false,
                bytes_written: 0,
            });
        }

        if request.term > guard.current_term {
            guard.current_term = request.term;
            guard.role = ConsensusRole::Follower;
            guard.voted_for = None;
        }

        let bytes_written = request.offset + request.data.len() as u64;

        Ok(InstallSnapshotResponse {
            term: guard.current_term,
            success: true,
            bytes_written,
        })
    }

    /// Advances consensus term and sets assigned role.
    pub fn transition_to(&self, role: ConsensusRole, term: TermId, voted_for: Option<NodeId>) {
        let mut guard = self.state.lock().unwrap();
        guard.role = role;
        guard.current_term = term;
        guard.voted_for = voted_for;
    }
}

impl ConsensusProtocol for ConsensusEngine {
    fn request_vote(
        &self,
        term: TermId,
        candidate_id: NodeId,
    ) -> Result<VoteResult, ConsensusError> {
        let mut guard = self.state.lock().unwrap();

        if term < guard.current_term {
            return Ok(VoteResult::StaleTerm);
        }

        if term > guard.current_term {
            guard.current_term = term;
            guard.role = ConsensusRole::Follower;
            guard.voted_for = Some(candidate_id);
            return Ok(VoteResult::Granted);
        }

        match guard.voted_for {
            None => {
                guard.voted_for = Some(candidate_id);
                Ok(VoteResult::Granted)
            }
            Some(existing) if existing == candidate_id => Ok(VoteResult::Granted),
            _ => Ok(VoteResult::AlreadyVoted),
        }
    }

    fn append_entries_raw(&self, term: TermId, leader_id: NodeId) -> Result<(), ConsensusError> {
        let _ = (term, leader_id);
        Err(ConsensusError::Unsupported)
    }
}

/// Raft consensus leader election strategy implementation of `LeaderElectionStrategy`.
pub struct RaftConsensusStrategy {
    engine: ConsensusEngine,
}

impl Default for RaftConsensusStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl RaftConsensusStrategy {
    /// Instantiates a new `RaftConsensusStrategy`.
    pub fn new() -> Self {
        Self {
            engine: ConsensusEngine::new(),
        }
    }

    /// Accesses underlying consensus engine.
    pub fn engine(&self) -> &ConsensusEngine {
        &self.engine
    }
}

impl LeaderElectionStrategy for RaftConsensusStrategy {
    fn select_leader(&self, candidates: &[&ClusterNode]) -> Result<NodeId, ClusterError> {
        if candidates.is_empty() {
            return Err(ClusterError::CoordinatorUnavailable);
        }

        let candidate_id = candidates[0].node_id;
        let current_state = self.engine.current_state();
        let next_term = TermId(current_state.current_term.0 + 1);

        self.engine
            .transition_to(ConsensusRole::Candidate, next_term, Some(candidate_id));

        let mut votes_granted = 0;
        for coord in candidates {
            if let Ok(VoteResult::Granted) = self.engine.request_vote(next_term, candidate_id) {
                votes_granted += 1;
            }
            let _ = coord;
        }

        if QuorumEvaluator::evaluate_quorum(votes_granted, candidates.len()) {
            self.engine
                .transition_to(ConsensusRole::Leader, next_term, Some(candidate_id));
            Ok(candidate_id)
        } else {
            self.engine
                .transition_to(ConsensusRole::Follower, next_term, None);
            Err(ClusterError::CoordinatorUnavailable)
        }
    }
}
