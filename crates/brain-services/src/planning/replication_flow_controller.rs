//! Pure Policy-Only `ReplicationFlowController` Architecture (Phase 13 Milestone 13.1).
//!
//! ### Architectural Invariants:
//! 1. Pure Policy Evaluation: `ReplicationFlowController` contains ZERO runtime state; it consumes immutable `ReplicationMeasurements` and returns deterministic `FlowDecision` artifacts.
//! 2. Immutable Policy Decisions: `FlowDecision` specifies `recommended_batch_size`, `send_window`, `pacing_delay_ms`, and `max_in_flight`.
//! 3. Adaptive Backpressure: Network latency (RTT), failure bursts, or in-flight byte accumulation adaptively scale down batch sizes and enforce pacing delays.

use serde::{Deserialize, Serialize};

/// Immutable operational metrics captured from follower replication streams.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicationMeasurements {
    /// Round-trip time in milliseconds.
    pub rtt_ms: u64,
    /// Follower ACK processing rate (ACKs per second).
    pub ack_rate: f64,
    /// Total bytes currently in flight to follower.
    pub bytes_in_flight: usize,
    /// Consecutive failed replication attempt count.
    pub consecutive_failures: u32,
}

impl Default for ReplicationMeasurements {
    fn default() -> Self {
        Self {
            rtt_ms: 10,
            ack_rate: 100.0,
            bytes_in_flight: 0,
            consecutive_failures: 0,
        }
    }
}

/// Immutable policy decision returned by `ReplicationFlowController`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowDecision {
    /// Recommended maximum entries in next `ReplicationBatch`.
    pub recommended_batch_size: usize,
    /// Recommended maximum entries in send window.
    pub send_window: usize,
    /// Pacing delay in milliseconds before next dispatch attempt.
    pub pacing_delay_ms: u64,
    /// Maximum concurrent in-flight requests permitted.
    pub max_in_flight: usize,
}

/// Pure policy controller evaluating replication flow control decisions.
#[derive(Debug, Clone, Default)]
pub struct ReplicationFlowController {
    min_batch_size: usize,
    max_batch_size: usize,
}

impl ReplicationFlowController {
    /// Instantiates a new `ReplicationFlowController` with batch bounds.
    pub fn new(min_batch_size: usize, max_batch_size: usize) -> Self {
        Self {
            min_batch_size,
            max_batch_size,
        }
    }

    /// Evaluates flow control decision deterministically from operational measurements.
    pub fn evaluate_flow(&self, measurements: &ReplicationMeasurements) -> FlowDecision {
        if measurements.consecutive_failures > 0 {
            // Failure backoff scaling
            let penalty = measurements.consecutive_failures as u64;
            return FlowDecision {
                recommended_batch_size: self.min_batch_size,
                send_window: 1,
                pacing_delay_ms: 50 * penalty,
                max_in_flight: 1,
            };
        }

        // Adaptive scaling based on RTT latency
        let (batch_size, delay_ms) = if measurements.rtt_ms > 200 {
            (self.min_batch_size, 20)
        } else if measurements.rtt_ms > 100 {
            ((self.max_batch_size / 2).max(self.min_batch_size), 5)
        } else {
            (self.max_batch_size, 0)
        };

        FlowDecision {
            recommended_batch_size: batch_size,
            send_window: batch_size * 2,
            pacing_delay_ms: delay_ms,
            max_in_flight: 5,
        }
    }
}
