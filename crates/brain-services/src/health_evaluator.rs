use serde::{Deserialize, Serialize};

/// Fine-grained derived health state evaluated dynamically from runtime telemetry snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DerivedRuntimeHealth {
    /// All runtime subsystems operating normally within health thresholds.
    Healthy,
    /// Non-critical performance degradation or backlog threshold breach.
    Degraded {
        /// Subsystem identifier (e.g., "projections", "orchestrator", "reflection").
        subsystem: String,
        /// Explanation of why the subsystem was classified as degraded.
        reason: String,
    },
    /// Critical runtime component failure requiring intervention.
    Unhealthy {
        /// Subsystem identifier.
        subsystem: String,
        /// Cause of unhealthy status.
        reason: String,
    },
}

/// Rule-based health evaluator operating on immutable point-in-time runtime snapshots.
#[derive(Debug, Clone)]
pub struct HealthEvaluator {
    /// Maximum allowed projection sequence lag before marking status degraded.
    pub max_projection_lag_threshold: u64,
    /// Maximum allowed consecutive task failure count before marking status degraded.
    pub max_failed_tasks_threshold: u64,
}

impl Default for HealthEvaluator {
    fn default() -> Self {
        Self {
            max_projection_lag_threshold: 1000,
            max_failed_tasks_threshold: 10,
        }
    }
}

impl HealthEvaluator {
    /// Evaluates derived system health dynamically from a `RuntimeDiagnosticsSnapshot`.
    pub fn evaluate(
        &self,
        pending_tasks: usize,
        failed_tasks: u64,
        max_projection_lag: u64,
    ) -> DerivedRuntimeHealth {
        if max_projection_lag > self.max_projection_lag_threshold {
            return DerivedRuntimeHealth::Degraded {
                subsystem: "projections".to_string(),
                reason: format!(
                    "Projection sequence lag ({}) exceeds threshold ({})",
                    max_projection_lag, self.max_projection_lag_threshold
                ),
            };
        }

        if failed_tasks > self.max_failed_tasks_threshold {
            return DerivedRuntimeHealth::Degraded {
                subsystem: "orchestrator".to_string(),
                reason: format!(
                    "Cumulative task failures ({}) exceed threshold ({})",
                    failed_tasks, self.max_failed_tasks_threshold
                ),
            };
        }

        if pending_tasks > 500 {
            return DerivedRuntimeHealth::Degraded {
                subsystem: "orchestrator".to_string(),
                reason: format!("Pending task queue depth ({}) is high", pending_tasks),
            };
        }

        DerivedRuntimeHealth::Healthy
    }
}
