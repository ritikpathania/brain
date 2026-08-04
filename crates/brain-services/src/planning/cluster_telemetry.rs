//! Production Telemetry, Configurable SLA/SLO Monitoring & Cluster Dashboard (Phase 15 Milestone 15.3).
//!
//! ### Architectural Invariants:
//! 1. Facts vs Policy Separation: `ClusterTelemetryProjection` stores raw objective facts without embedding health or SLA rules.
//! 2. Configurable SLO Policies: `SlaSloMonitor` evaluates configurable `SloPolicy` specifications deterministically.
//! 3. Lazy Derived Metrics: Average latencies and error budget percentages are derived lazily from raw cumulative totals.

use crate::planning::durable_event_store::EventEnvelope;
use crate::planning::log_replay_engine::ReplayTarget;
use crate::planning::read_events::{ReadEvent, ReadEventKind};
use crate::planning::replication_events::{ReplicationEvent, ReplicationEventKind};
use serde::{Deserialize, Serialize};

/// Objective cumulative facts collected by telemetry projections.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ClusterTelemetryMetrics {
    /// Total operations processed.
    pub total_requests: u64,
    /// Total successful operations.
    pub successful_requests: u64,
    /// Total failed operations.
    pub failed_requests: u64,
    /// Cumulative latency across all operations in microseconds.
    pub total_latency_us: u64,
    /// Count of distinct active cluster nodes observed.
    pub active_nodes: u64,
}

impl ClusterTelemetryMetrics {
    /// Computes lazy availability percentage (0.0 to 100.0).
    pub fn availability_pct(&self) -> f64 {
        if self.total_requests == 0 {
            100.0
        } else {
            (self.successful_requests as f64 / self.total_requests as f64) * 100.0
        }
    }

    /// Computes lazy average operation latency in microseconds.
    pub fn average_latency_us(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_latency_us as f64 / self.total_requests as f64
        }
    }
}

/// Event-sourced telemetry projection aggregating read and replication events.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTelemetryProjection {
    /// Accumulated raw telemetry facts.
    pub metrics: ClusterTelemetryMetrics,
}

impl ClusterTelemetryProjection {
    /// Instantiates a new `ClusterTelemetryProjection`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ReplayTarget<ReadEvent> for ClusterTelemetryProjection {
    fn apply_envelope(&mut self, env: &EventEnvelope<ReadEvent>) {
        let evt = &env.payload;
        match &evt.kind {
            ReadEventKind::ReadRequested { .. } => {
                self.metrics.total_requests += 1;
            }
            ReadEventKind::ReadServed {
                planning_latency_us,
                validation_latency_us,
                execution_latency_us,
                ..
            } => {
                self.metrics.successful_requests += 1;
                self.metrics.total_latency_us +=
                    planning_latency_us + validation_latency_us + execution_latency_us;
            }
            ReadEventKind::ReadRejected { .. } => {
                self.metrics.failed_requests += 1;
            }
            _ => {}
        }
    }
}

impl ReplayTarget<ReplicationEvent> for ClusterTelemetryProjection {
    fn apply_envelope(&mut self, env: &EventEnvelope<ReplicationEvent>) {
        let evt = &env.payload;
        match &evt.kind {
            ReplicationEventKind::BatchSent { entry_count, .. } => {
                self.metrics.total_requests += *entry_count as u64;
            }
            ReplicationEventKind::AckReceived { rtt_ms, .. } => {
                self.metrics.successful_requests += 1;
                self.metrics.total_latency_us += rtt_ms * 1000;
            }
            ReplicationEventKind::RetryScheduled { .. } => {
                self.metrics.failed_requests += 1;
            }
            _ => {}
        }
    }
}

/// Configurable operational Service Level Objective (SLO) specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SloPolicy {
    /// Target minimum availability percentage (e.g., 99.9).
    pub target_availability_pct: f64,
    /// Target maximum average latency threshold in microseconds (e.g., 50000.0).
    pub max_average_latency_us: f64,
}

impl Default for SloPolicy {
    fn default() -> Self {
        Self {
            target_availability_pct: 99.9,
            max_average_latency_us: 50_000.0,
        }
    }
}

/// Evaluation report derived from evaluating an `SloPolicy` against telemetry metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SloEvaluationReport {
    /// Calculated operational availability percentage.
    pub actual_availability_pct: f64,
    /// Calculated average operation latency in microseconds.
    pub actual_average_latency_us: f64,
    /// `true` if all targets specified by `SloPolicy` are satisfied.
    pub slo_met: bool,
    /// Remaining error budget percentage (0.0 to 100.0).
    pub error_budget_remaining_pct: f64,
}

/// Pure policy evaluation engine for SLA/SLO monitoring.
pub struct SlaSloMonitor;

impl SlaSloMonitor {
    /// Evaluates `SloPolicy` targets against `ClusterTelemetryMetrics` deterministically.
    pub fn evaluate_slo(
        metrics: &ClusterTelemetryMetrics,
        policy: &SloPolicy,
    ) -> SloEvaluationReport {
        let actual_avail = metrics.availability_pct();
        let actual_latency = metrics.average_latency_us();

        let avail_ok = actual_avail >= policy.target_availability_pct;
        let latency_ok =
            metrics.total_requests == 0 || actual_latency <= policy.max_average_latency_us;
        let slo_met = avail_ok && latency_ok;

        let allowed_failure_rate = 100.0 - policy.target_availability_pct;
        let actual_failure_rate = 100.0 - actual_avail;

        let error_budget_remaining_pct = if allowed_failure_rate <= 0.0 {
            if actual_failure_rate > 0.0 {
                0.0
            } else {
                100.0
            }
        } else {
            let remaining =
                ((allowed_failure_rate - actual_failure_rate) / allowed_failure_rate) * 100.0;
            remaining.clamp(0.0, 100.0)
        };

        SloEvaluationReport {
            actual_availability_pct: actual_avail,
            actual_average_latency_us: actual_latency,
            slo_met,
            error_budget_remaining_pct,
        }
    }
}

/// Operator presentation dashboard snapshot compiling telemetry and SLO evaluation results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterHealthDashboard {
    /// Telemetry metrics snapshot.
    pub metrics: ClusterTelemetryMetrics,
    /// Associated SLO evaluation report.
    pub slo_report: SloEvaluationReport,
}

impl ClusterHealthDashboard {
    /// Compiles a new dashboard snapshot from telemetry metrics and an `SloPolicy`.
    pub fn compile(metrics: ClusterTelemetryMetrics, policy: &SloPolicy) -> Self {
        let slo_report = SlaSloMonitor::evaluate_slo(&metrics, policy);
        Self {
            metrics,
            slo_report,
        }
    }
}
