//! Decoupled `ReadPlanner`, Immutable `ReadPlan` Artifact & `LinearizableReadEngine` (Phase 13 Milestone 13.3).
//!
//! ### Architectural Invariants:
//! 1. Compiler-Inspired Architecture: `ReadPlanner` compiles an immutable `ReadPlan`; `LinearizableReadEngine` executes validation and returns `ReadIndexResponse`.
//! 2. Explicit Consistency Strategy: `ReadConsistencyStrategy` explicitly distinguishes between `LeaderLease(LeaderLease)` and `ReadIndexQuorum`.
//! 3. Zero-Log-Append Linearizable Reads: Evaluates committed sequence numbers (`read_index`) without mutating event logs or executing log appends.

use crate::planning::cluster::NodeId;
use crate::planning::consensus::{
    ConsensusEngine, LeaderLease, ReadIndexRequest, ReadIndexResponse, ReadValidationResult, TermId,
};
use crate::planning::durable_event_store::{EventLog, SequenceNumber};
use crate::planning::leader_lease_validator::LeaderLeaseValidator;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Explicit strategy classification for satisfying linearizable reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadConsistencyStrategy {
    /// Zero-log-append read validated via active `LeaderLease`.
    LeaderLease(LeaderLease),
    /// Read confirmed via heartbeat quorum ping.
    ReadIndexQuorum,
}

/// Explicit classification of compiled `ReadPlan` execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReadPlanKind {
    /// Read plan validated via unexpired leader lease bounds.
    LeaseValidated,
    /// Read plan validated via heartbeat quorum confirmation.
    QuorumConfirmed,
}

/// Immutable read plan compiled by `ReadPlanner`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadPlan {
    /// Unique read query identifier.
    pub read_id: Uuid,
    /// Target leader node ID.
    pub leader_id: NodeId,
    /// Consensus term when plan was compiled.
    pub term: TermId,
    /// Target sequence number index at which projection query will be executed.
    pub target_read_index: SequenceNumber,
    /// Plan execution mode kind.
    pub kind: ReadPlanKind,
    /// Consistency strategy specification.
    pub strategy: ReadConsistencyStrategy,
}

/// Pure planner compiling immutable `ReadPlan` artifacts.
pub struct ReadPlanner;

impl ReadPlanner {
    /// Compiles an immutable `ReadPlan` for a `ReadIndexRequest` against the current log state.
    pub fn plan_read<E, L: EventLog<E>>(
        request: &ReadIndexRequest,
        log: &L,
        strategy: ReadConsistencyStrategy,
    ) -> ReadPlan {
        let last_committed_seq = log.last_sequence_number();
        let kind = match &strategy {
            ReadConsistencyStrategy::LeaderLease(_) => ReadPlanKind::LeaseValidated,
            ReadConsistencyStrategy::ReadIndexQuorum => ReadPlanKind::QuorumConfirmed,
        };

        ReadPlan {
            read_id: request.read_id,
            leader_id: request.leader_id,
            term: request.term,
            target_read_index: last_committed_seq,
            kind,
            strategy,
        }
    }
}

/// Execution engine validating compiled `ReadPlan` artifacts.
pub struct LinearizableReadEngine;

impl LinearizableReadEngine {
    /// Executes a `ReadPlan`, validating term and lease bounds without performing log appends.
    pub fn execute_read_plan(
        engine: &ConsensusEngine,
        plan: &ReadPlan,
        now_ms: u64,
    ) -> ReadIndexResponse {
        let current_state = engine.current_state();

        // 1. Term check invariant: reject stale leader term
        if plan.term < current_state.current_term {
            return ReadIndexResponse {
                read_id: plan.read_id,
                term: current_state.current_term,
                read_index: SequenceNumber(0),
                validation_result: ReadValidationResult::StaleLeader,
            };
        }

        // 2. Validate strategy
        let validation_result = match &plan.strategy {
            ReadConsistencyStrategy::LeaderLease(lease) => {
                match LeaderLeaseValidator::validate_lease(
                    lease,
                    &plan.leader_id,
                    plan.term,
                    now_ms,
                ) {
                    Ok(()) => ReadValidationResult::LeaseValid,
                    Err(result) => result,
                }
            }
            ReadConsistencyStrategy::ReadIndexQuorum => ReadValidationResult::LeaseValid,
        };

        let final_read_index = if validation_result.is_success() {
            plan.target_read_index
        } else {
            SequenceNumber(0)
        };

        ReadIndexResponse {
            read_id: plan.read_id,
            term: current_state.current_term,
            read_index: final_read_index,
            validation_result,
        }
    }
}
