//! Quality measurements, quality dimensions, and QualityScorecard evaluation gates.

use brain_domain::EvaluationMetrics;
use std::fmt;

/// Semantic quality classification dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum QualityDimension {
    /// Reasoning correctness quality dimension.
    Correctness,
    /// Execution replay determinism dimension.
    Determinism,
    /// Execution latency & performance budget dimension.
    Performance,
    /// Retrieval precision and recall dimension.
    Retrieval,
    /// Memory stewardship integrity dimension.
    Stewardship,
}

impl fmt::Display for QualityDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Correctness => write!(f, "Correctness"),
            Self::Determinism => write!(f, "Determinism"),
            Self::Performance => write!(f, "Performance"),
            Self::Retrieval => write!(f, "Retrieval"),
            Self::Stewardship => write!(f, "Stewardship"),
        }
    }
}

/// Raw numerical quality measurements decoupling raw statistics from pass/fail judgments.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QualityMeasurements {
    /// Latency measurement in milliseconds.
    pub latency_ms: u64,
    /// Replay determinism score (1.0 = exact match).
    pub determinism_score: f32,
    /// Retrieval precision score (0.0 to 1.0).
    pub precision_score: f32,
    /// Retrieval recall score (0.0 to 1.0).
    pub recall_score: f32,
}

impl QualityMeasurements {
    /// Instantiates a new `QualityMeasurements`.
    pub fn new(
        latency_ms: u64,
        determinism_score: f32,
        precision_score: f32,
        recall_score: f32,
    ) -> Self {
        Self {
            latency_ms,
            determinism_score,
            precision_score,
            recall_score,
        }
    }
}

/// Evaluation scorecard applying threshold checks over raw `QualityMeasurements`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QualityScorecard {
    /// Measured quality dimensions.
    pub measurements: QualityMeasurements,
    /// Evaluated pass/fail status.
    pub scorecard_passed: bool,
}

impl QualityScorecard {
    /// Evaluates raw `QualityMeasurements` against quality gate thresholds.
    pub fn evaluate(measurements: QualityMeasurements) -> Self {
        let scorecard_passed = measurements.determinism_score >= 1.0
            && measurements.precision_score >= 0.7
            && measurements.recall_score >= 0.7;

        Self {
            measurements,
            scorecard_passed,
        }
    }

    /// Evaluates evaluation metrics into a quality scorecard.
    pub fn from_eval_metrics(metrics: &EvaluationMetrics) -> Self {
        let measurements = QualityMeasurements::new(0, metrics.determinism_score, 1.0, 1.0);
        Self::evaluate(measurements)
    }
}
