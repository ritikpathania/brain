//! Suggested action recommendations for stewardship findings.

use super::finding::FindingId;
use serde::{Deserialize, Serialize};

/// Action classification for a stewardship recommendation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum RecommendationKind {
    /// Combine duplicate memories/documents into a single record.
    Merge,
    /// Archive outdated or superseded knowledge context.
    Archive,
    /// Update memory properties with newer authoritative facts.
    Update,
    /// Explicitly ignore an observed finding.
    Ignore,
    /// Flag for manual human investigation.
    #[default]
    Investigate,
}

/// Opaque newtype identifier for a stewardship recommendation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RecommendationId(pub uuid::Uuid);

impl Default for RecommendationId {
    fn default() -> Self {
        Self::new()
    }
}

impl RecommendationId {
    /// Generates a new random RecommendationId.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for RecommendationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rec-{}", self.0)
    }
}

/// Suggested action recommendation for a finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StewardshipRecommendation {
    /// Unique recommendation identifier.
    pub id: RecommendationId,
    /// Associated finding identifier.
    pub finding_id: FindingId,
    /// Suggested action kind.
    pub kind: RecommendationKind,
    /// Rationale explaining why this action is recommended.
    pub rationale: String,
}

impl StewardshipRecommendation {
    /// Creates a new StewardshipRecommendation.
    pub fn new(
        finding_id: FindingId,
        kind: RecommendationKind,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            id: RecommendationId::new(),
            finding_id,
            kind,
            rationale: rationale.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommendation_construction() {
        let finding_id = FindingId::new();
        let rec = StewardshipRecommendation::new(
            finding_id,
            RecommendationKind::Merge,
            "High semantic similarity overlap",
        );

        assert_eq!(rec.finding_id, finding_id);
        assert_eq!(rec.kind, RecommendationKind::Merge);
    }
}
