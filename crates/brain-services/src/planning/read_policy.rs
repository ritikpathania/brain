//! Pluggable `ReadPolicy` Trait & Built-in Consistency Selection Strategies (Phase 14 Milestone 14.4).
//!
//! ### Architectural Invariants:
//! 1. Planner / Executor Split Preservation: `ReadPolicy` selects `ReadConsistencyStrategy` without modifying `ReadPlanner` or `LinearizableReadEngine`.
//! 2. Pure Policy Evaluation: Policy selection evaluates request metadata and lease state deterministically without side effects.
//! 3. Fallback Safety: `LeasePriorityPolicy` validates lease bounds via `LeaderLeaseValidator` and falls back safely to `ReadIndexQuorum` if lease is expired or missing.

use crate::planning::consensus::{LeaderLease, ReadIndexRequest};
use crate::planning::leader_lease_validator::LeaderLeaseValidator;
use crate::planning::linearizable_read_engine::ReadConsistencyStrategy;

/// Pluggable policy interface for selecting read consistency strategies for `ReadIndexRequest` queries.
pub trait ReadPolicy: Send + Sync {
    /// Selects a `ReadConsistencyStrategy` for a `ReadIndexRequest` query.
    fn select_strategy(
        &self,
        request: &ReadIndexRequest,
        leader_lease: Option<&LeaderLease>,
        now_ms: u64,
    ) -> ReadConsistencyStrategy;
}

/// Policy preferring `LeaderLease` if unexpired and matching, falling back to `ReadIndexQuorum`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LeasePriorityPolicy;

impl LeasePriorityPolicy {
    /// Instantiates a new `LeasePriorityPolicy`.
    pub fn new() -> Self {
        Self
    }
}

impl ReadPolicy for LeasePriorityPolicy {
    fn select_strategy(
        &self,
        request: &ReadIndexRequest,
        leader_lease: Option<&LeaderLease>,
        now_ms: u64,
    ) -> ReadConsistencyStrategy {
        if let Some(lease) = leader_lease {
            if LeaderLeaseValidator::validate_lease(lease, &request.leader_id, request.term, now_ms)
                .is_ok()
            {
                return ReadConsistencyStrategy::LeaderLease(*lease);
            }
        }
        ReadConsistencyStrategy::ReadIndexQuorum
    }
}

/// Policy strictly enforcing heartbeat quorum confirmation for every linearizable read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuorumOnlyPolicy;

impl QuorumOnlyPolicy {
    /// Instantiates a new `QuorumOnlyPolicy`.
    pub fn new() -> Self {
        Self
    }
}

impl ReadPolicy for QuorumOnlyPolicy {
    fn select_strategy(
        &self,
        _request: &ReadIndexRequest,
        _leader_lease: Option<&LeaderLease>,
        _now_ms: u64,
    ) -> ReadConsistencyStrategy {
        ReadConsistencyStrategy::ReadIndexQuorum
    }
}

/// Pure policy evaluator driving strategy selection from a `ReadPolicy` implementation.
pub struct ReadPolicyEvaluator;

impl ReadPolicyEvaluator {
    /// Evaluates read strategy selection deterministically.
    pub fn evaluate_policy<P: ReadPolicy>(
        policy: &P,
        request: &ReadIndexRequest,
        leader_lease: Option<&LeaderLease>,
        now_ms: u64,
    ) -> ReadConsistencyStrategy {
        policy.select_strategy(request, leader_lease, now_ms)
    }
}
