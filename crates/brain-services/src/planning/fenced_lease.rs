//! `FencedLease` and Distributed Lease Fencing (Phase 10 Milestone 10.1).
//!
//! ### Architectural Invariants:
//! 1. Anti-Zombie Protection: `FencedLease` wraps a `WorkerLease` with an `EpochId` and a monotonically increasing `fence_token`.
//! 2. Verification Invariant: `verify_fence_token` rejects stale/old fence tokens with `ClusterError::InvalidFenceToken`.
//! 3. Fence Token Monotonicity: Fence tokens strictly increase across reassignments; tokens are never reused.

use crate::planning::cluster::{ClusterError, EpochId};
use crate::planning::scheduler::WorkerLease;
use serde::{Deserialize, Serialize};

/// Distributed fenced lease wrapper enforcing epoch and fence token validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FencedLease {
    /// Wrapped worker lease.
    pub lease: WorkerLease,
    /// Associated cluster epoch.
    pub epoch: EpochId,
    /// Monotonically increasing fence token.
    pub fence_token: u64,
}

impl FencedLease {
    /// Instantiates a new `FencedLease`.
    pub fn new(lease: WorkerLease, epoch: EpochId, fence_token: u64) -> Self {
        Self {
            lease,
            epoch,
            fence_token,
        }
    }

    /// Verifies that a provided fence token satisfies the expected minimum fence token bound.
    pub fn verify_fence_token(&self, expected_min_token: u64) -> Result<(), ClusterError> {
        if self.fence_token < expected_min_token {
            Err(ClusterError::InvalidFenceToken {
                expected: expected_min_token,
                found: self.fence_token,
            })
        } else {
            Ok(())
        }
    }
}
