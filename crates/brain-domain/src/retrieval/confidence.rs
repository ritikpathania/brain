//! Structured confidence levels and assessments for cognitive retrieval results.

use serde::{Deserialize, Serialize};

/// Categorical confidence levels indicating overall retrieval accuracy.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum ConfidenceLevel {
    /// High confidence backed by critical evidence match.
    #[default]
    High,
    /// Medium confidence with good semantic alignment.
    Medium,
    /// Low confidence with partial keyword overlap.
    Low,
    /// Uncertain confidence requiring user discretion.
    Uncertain,
}

/// Structured confidence assessment containing float score and categorical level.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceAssessment {
    /// Numeric confidence score in range [0.0, 1.0].
    pub score: f32,
    /// Discrete confidence level.
    pub level: ConfidenceLevel,
}

impl Default for ConfidenceAssessment {
    fn default() -> Self {
        Self {
            score: 1.0,
            level: ConfidenceLevel::High,
        }
    }
}

impl ConfidenceAssessment {
    /// Creates a new ConfidenceAssessment clamping the score to [0.0, 1.0].
    pub fn new(score: f32) -> Self {
        let clamped = score.clamp(0.0, 1.0);
        let level = if clamped >= 0.85 {
            ConfidenceLevel::High
        } else if clamped >= 0.65 {
            ConfidenceLevel::Medium
        } else if clamped >= 0.40 {
            ConfidenceLevel::Low
        } else {
            ConfidenceLevel::Uncertain
        };

        Self {
            score: clamped,
            level,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_level_mapping() {
        assert_eq!(ConfidenceAssessment::new(0.92).level, ConfidenceLevel::High);
        assert_eq!(
            ConfidenceAssessment::new(0.70).level,
            ConfidenceLevel::Medium
        );
        assert_eq!(ConfidenceAssessment::new(0.50).level, ConfidenceLevel::Low);
        assert_eq!(
            ConfidenceAssessment::new(0.20).level,
            ConfidenceLevel::Uncertain
        );
    }
}
