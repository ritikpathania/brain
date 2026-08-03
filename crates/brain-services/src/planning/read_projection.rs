//! Event-Sourced `ReadProjection` & Extensible `ReadMetrics` (Phase 14 Milestone 14.2).
//!
//! ### Architectural Invariants:
//! 1. Lazy Derived Statistics Only: Averages (`average_execution_latency_us`) are computed lazily on demand from raw cumulative totals; no stored average fields.
//! 2. Extensible Strategy Accounting: Strategy counts (`strategy_counts: HashMap<ReadPlanKind, u64>`) and rejections (`rejections_by_reason: HashMap<ReadValidationResult, u64>`) are stored in dynamic maps.
//! 3. Replay Monotonic Sequence Integrity: Enforces strict monotonic sequence checking during `apply_envelope`.

use crate::planning::consensus::ReadValidationResult;
use crate::planning::durable_event_store::{EventEnvelope, SequenceNumber};
use crate::planning::linearizable_read_engine::ReadPlanKind;
use crate::planning::log_replay_engine::ReplayTarget;
use crate::planning::read_events::{ReadEvent, ReadEventKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Operational metrics derived from replaying linearizable read events.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReadMetrics {
    /// Total read requests processed.
    pub total_reads: u64,
    /// Total linearizable reads successfully served.
    pub successful_reads: u64,
    /// Total linearizable reads rejected.
    pub rejected_reads: u64,
    /// Cumulative planning stage duration in microseconds.
    pub total_planning_latency_us: u64,
    /// Cumulative validation stage duration in microseconds.
    pub total_validation_latency_us: u64,
    /// Cumulative execution stage duration in microseconds.
    pub total_execution_latency_us: u64,
    /// Strategy accounting breakdown.
    pub strategy_counts: HashMap<ReadPlanKind, u64>,
    /// Rejection counts grouped by diagnostic reason.
    pub rejections_by_reason: HashMap<ReadValidationResult, u64>,
}

impl ReadMetrics {
    /// Computes lazy average execution latency in microseconds (returns 0.0 if no successful reads).
    pub fn average_execution_latency_us(&self) -> f64 {
        if self.successful_reads == 0 {
            0.0
        } else {
            self.total_execution_latency_us as f64 / self.successful_reads as f64
        }
    }

    /// Computes lazy average total end-to-end latency in microseconds.
    pub fn average_total_latency_us(&self) -> f64 {
        if self.successful_reads == 0 {
            0.0
        } else {
            let total = self.total_planning_latency_us
                + self.total_validation_latency_us
                + self.total_execution_latency_us;
            total as f64 / self.successful_reads as f64
        }
    }
}

/// Event-sourced projection tracking linearizable read metrics and strategy statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadProjection {
    /// Accumulated read metrics.
    pub metrics: ReadMetrics,
    /// Highest sequence number processed by projection.
    pub last_sequence: SequenceNumber,
    /// Count of sequence regression anomalies detected during replay.
    pub sequence_regressions: u64,
    /// Count of sequence gap anomalies detected during replay.
    pub sequence_gaps: u64,
}

impl Default for ReadProjection {
    fn default() -> Self {
        Self {
            metrics: ReadMetrics::default(),
            last_sequence: SequenceNumber(0),
            sequence_regressions: 0,
            sequence_gaps: 0,
        }
    }
}

impl ReadProjection {
    /// Instantiates a new `ReadProjection`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns last sequence index applied to projection.
    pub fn last_applied_sequence(&self) -> SequenceNumber {
        self.last_sequence
    }
}

impl ReplayTarget<ReadEvent> for ReadProjection {
    fn apply_envelope(&mut self, env: &EventEnvelope<ReadEvent>) {
        if self.last_sequence.0 > 0 {
            if env.sequence.0 == self.last_sequence.0 {
                // Exact duplicate sequence -> Ignore silently for idempotency
                return;
            }
            if env.sequence.0 < self.last_sequence.0 {
                // Sequence regression anomaly
                self.sequence_regressions += 1;
                return;
            }
            if env.sequence.0 > self.last_sequence.0 + 1 {
                // Sequence gap anomaly
                self.sequence_gaps += 1;
            }
        }

        self.last_sequence = env.sequence;
        let evt = &env.payload;

        match &evt.kind {
            ReadEventKind::ReadRequested { .. } => {
                self.metrics.total_reads += 1;
            }
            ReadEventKind::ReadPlanCompiled { kind, .. } => {
                *self.metrics.strategy_counts.entry(*kind).or_insert(0) += 1;
            }
            ReadEventKind::ReadServed {
                planning_latency_us,
                validation_latency_us,
                execution_latency_us,
                ..
            } => {
                self.metrics.successful_reads += 1;
                self.metrics.total_planning_latency_us += planning_latency_us;
                self.metrics.total_validation_latency_us += validation_latency_us;
                self.metrics.total_execution_latency_us += execution_latency_us;
            }
            ReadEventKind::ReadRejected { reason } => {
                self.metrics.rejected_reads += 1;
                *self
                    .metrics
                    .rejections_by_reason
                    .entry(*reason)
                    .or_insert(0) += 1;
            }
            _ => {}
        }
    }
}
