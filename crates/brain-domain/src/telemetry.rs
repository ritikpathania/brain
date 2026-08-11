//! Operational telemetry, evaluation metrics, runtime event stream, and runtime policy configuration.

use crate::execution::ExecutionId;
use std::fmt;

/// Operational phase durations measured in milliseconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PhaseDurationMs {
    /// Synthesis phase duration.
    pub synthesis_ms: u64,
    /// Reflection phase duration.
    pub reflection_ms: u64,
    /// Matching phase duration.
    pub matching_ms: u64,
    /// Consolidation phase duration.
    pub consolidation_ms: u64,
    /// Stewardship execution phase duration.
    pub stewardship_ms: u64,
}

/// Operational telemetry capturing execution counts and timing metrics.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OperationalTelemetry {
    /// Target execution run ID.
    pub execution_id: ExecutionId,
    /// Total duration in milliseconds.
    pub total_duration_ms: u64,
    /// Phase breakdown durations.
    pub phase_durations: PhaseDurationMs,
    /// Extracted candidates count.
    pub extracted_candidates_count: usize,
    /// Promoted entities count.
    pub promoted_entities_count: usize,
    /// Rejected duplicate candidates count.
    pub rejected_duplicates_count: usize,
}

impl OperationalTelemetry {
    /// Instantiates a new `OperationalTelemetry`.
    pub fn new(execution_id: ExecutionId) -> Self {
        Self {
            execution_id,
            total_duration_ms: 0,
            phase_durations: PhaseDurationMs::default(),
            extracted_candidates_count: 0,
            promoted_entities_count: 0,
            rejected_duplicates_count: 0,
        }
    }
}

/// Benchmark evaluation metrics evaluating runtime correctness and determinism.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvaluationMetrics {
    /// Replay success boolean.
    pub replay_successful: bool,
    /// Replay determinism score (1.0 = exact match).
    pub determinism_score: f32,
    /// Total regression checks passed.
    pub regression_checks_passed: usize,
}

impl EvaluationMetrics {
    /// Instantiates a new `EvaluationMetrics`.
    pub fn new(
        replay_successful: bool,
        determinism_score: f32,
        regression_checks_passed: usize,
    ) -> Self {
        Self {
            replay_successful,
            determinism_score,
            regression_checks_passed,
        }
    }
}

/// Real-time lifecycle event emitted during reasoning cycle execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RuntimeEvent {
    /// Cycle execution started.
    ExecutionStarted {
        /// Target execution run ID.
        execution_id: ExecutionId,
    },
    /// Synthesis phase completed.
    ReasoningCompleted {
        /// Target execution run ID.
        execution_id: ExecutionId,
    },
    /// Reflection phase completed.
    ReflectionCompleted {
        /// Target execution run ID.
        execution_id: ExecutionId,
    },
    /// Knowledge candidates extracted.
    CandidateExtractionCompleted {
        /// Target execution run ID.
        execution_id: ExecutionId,
        /// Extracted candidates count.
        candidate_count: usize,
    },
    /// Graph matching completed.
    MatchingCompleted {
        /// Target execution run ID.
        execution_id: ExecutionId,
    },
    /// Consolidation phase completed.
    ConsolidationCompleted {
        /// Target execution run ID.
        execution_id: ExecutionId,
    },
    /// Memory stewardship completed.
    StewardshipCompleted {
        /// Target execution run ID.
        execution_id: ExecutionId,
        /// Succeeded mutation count.
        succeeded_count: usize,
    },
    /// Full runtime cycle completed.
    RuntimeCompleted {
        /// Target execution run ID.
        execution_id: ExecutionId,
    },
}

/// Immutable runtime configuration policy set selected at execution start.
/// Invariant: Every RuntimeExecutionReport must record the policy configuration used to produce it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimePolicySet {
    /// Unique policy set version identifier.
    pub policy_version: String,
    /// Minimum graph similarity threshold float (0.0 to 1.0).
    pub min_similarity_threshold: u32,
}

impl RuntimePolicySet {
    /// Default standard runtime policy set.
    pub fn default_standard() -> Self {
        Self {
            policy_version: "v1.0.0-default".to_string(),
            min_similarity_threshold: 80,
        }
    }

    /// Strict high-threshold runtime policy set.
    pub fn strict_policy() -> Self {
        Self {
            policy_version: "v1.0.0-strict".to_string(),
            min_similarity_threshold: 95,
        }
    }
}

impl Default for RuntimePolicySet {
    fn default() -> Self {
        Self::default_standard()
    }
}

impl fmt::Display for RuntimePolicySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PolicySet({})", self.policy_version)
    }
}
