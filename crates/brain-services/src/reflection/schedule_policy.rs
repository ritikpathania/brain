//! Data-driven composable schedule policies and triggers for reflection execution.

use crate::reflection::contracts::ReflectionExecutionMode;

/// Individual composable condition for triggering a reflection run.
#[derive(Debug, Clone, PartialEq)]
pub enum ScheduleTrigger {
    /// Triggered when elapsed time since last run exceeds threshold.
    ElapsedTime {
        /// Threshold duration in milliseconds.
        duration_ms: u64,
    },
    /// Triggered when new un-reflected observations exceed threshold.
    ObservationCount {
        /// Threshold observation count.
        threshold: usize,
    },
    /// Triggered when graph topology drift score exceeds threshold.
    GraphDrift {
        /// Threshold drift score (0.0 to 1.0).
        drift_score: f32,
    },
    /// Triggered when pending merge proposals exceed threshold.
    PendingMerges {
        /// Threshold pending merge proposal count.
        count: usize,
    },
}

/// Dynamic snapshot of current system state passed to `SchedulePolicy`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SystemMetricsSnapshot {
    /// Time in milliseconds since the last reflection run completed.
    pub elapsed_since_last_run_ms: u64,
    /// Count of observations ingested since last run.
    pub pending_observation_count: usize,
    /// Measured graph drift score (0.0 to 1.0).
    pub current_graph_drift: f32,
    /// Count of unapproved merge proposals pending review.
    pub pending_merge_proposal_count: usize,
}

/// Composable schedule policy evaluating trigger rules against system metrics.
#[derive(Debug, Clone, Default)]
pub struct SchedulePolicy {
    triggers: Vec<ScheduleTrigger>,
}

impl SchedulePolicy {
    /// Creates a new empty `SchedulePolicy`.
    pub fn new() -> Self {
        Self {
            triggers: Vec::new(),
        }
    }

    /// Adds a trigger condition to the policy.
    pub fn add_trigger(mut self, trigger: ScheduleTrigger) -> Self {
        self.triggers.push(trigger);
        self
    }

    /// Evaluates if any composable trigger rule is satisfied given current system metrics.
    pub fn should_trigger(
        &self,
        snapshot: &SystemMetricsSnapshot,
    ) -> Option<ReflectionExecutionMode> {
        for trigger in &self.triggers {
            match trigger {
                ScheduleTrigger::ElapsedTime { duration_ms } => {
                    if snapshot.elapsed_since_last_run_ms >= *duration_ms {
                        return Some(ReflectionExecutionMode::Periodic);
                    }
                }
                ScheduleTrigger::ObservationCount { threshold } => {
                    if snapshot.pending_observation_count >= *threshold {
                        return Some(ReflectionExecutionMode::Idle);
                    }
                }
                ScheduleTrigger::GraphDrift { drift_score } => {
                    if snapshot.current_graph_drift >= *drift_score {
                        return Some(ReflectionExecutionMode::Dream);
                    }
                }
                ScheduleTrigger::PendingMerges { count } => {
                    if snapshot.pending_merge_proposal_count >= *count {
                        return Some(ReflectionExecutionMode::Manual);
                    }
                }
            }
        }
        None
    }
}
