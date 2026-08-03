//! Pure `LeaderLeaseValidator` Component for Read-Lease Time Bounds Verification (Phase 13 Milestone 13.3).
//!
//! ### Architectural Invariants:
//! 1. Single Responsibility: `LeaderLeaseValidator` validates leader identity, term monotonicity, and TTL time bounds without executing reads.
//! 2. Explicit Diagnostic Errors: Returns strongly-typed `ReadValidationResult` variants (`LeaseExpired`, `StaleLeader`).

use crate::planning::cluster::NodeId;
use crate::planning::consensus::{LeaderLease, ReadValidationResult, TermId};

/// Pure validator evaluating `LeaderLease` time bounds and term integrity.
pub struct LeaderLeaseValidator;

impl LeaderLeaseValidator {
    /// Validates whether a `LeaderLease` is active, unexpired, and matches expected leader and term.
    pub fn validate_lease(
        lease: &LeaderLease,
        expected_leader: &NodeId,
        expected_term: TermId,
        now_ms: u64,
    ) -> Result<(), ReadValidationResult> {
        if lease.leader_id != *expected_leader || lease.term != expected_term {
            return Err(ReadValidationResult::StaleLeader);
        }

        if now_ms >= lease.granted_at_ms + lease.lease_ttl_ms {
            return Err(ReadValidationResult::LeaseExpired);
        }

        Ok(())
    }
}
