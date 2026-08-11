//! Domain representation of stewardship findings and observations.

use crate::identifiers::SourceId;
use crate::retrieval::ConfidenceAssessment;
use serde::{Deserialize, Serialize};

/// Semantic category classification of an observed stewardship finding.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum FindingKind {
    /// Discrepancy or conflicting facts between documents/memories.
    #[default]
    Contradiction,
    /// Outdated, expired, or superseded memory context.
    Staleness,
    /// High semantic similarity duplicate candidate.
    Duplication,
    /// Missing relationship links or orphan entity vertices.
    Incompleteness,
}

/// Opaque newtype identifier for a stewardship finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FindingId(pub uuid::Uuid);

impl Default for FindingId {
    fn default() -> Self {
        Self::new()
    }
}

impl FindingId {
    /// Generates a new random FindingId.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for FindingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "finding-{}", self.0)
    }
}

/// Domain finding aggregate describing an observation in the knowledge base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StewardshipFinding {
    /// Unique finding identifier.
    pub id: FindingId,
    /// Finding classification category.
    pub kind: FindingKind,
    /// Human-readable summary of the finding.
    pub summary: String,
    /// Detailed factual description of what was observed.
    pub description: String,
    /// Referenced source asset identifiers.
    pub affected_sources: Vec<SourceId>,
    /// Confidence assessment of the observation.
    pub confidence: ConfidenceAssessment,
}

impl StewardshipFinding {
    /// Creates a new StewardshipFinding.
    pub fn new(
        kind: FindingKind,
        summary: impl Into<String>,
        description: impl Into<String>,
        affected_sources: Vec<SourceId>,
        confidence: ConfidenceAssessment,
    ) -> Self {
        Self {
            id: FindingId::new(),
            kind,
            summary: summary.into(),
            description: description.into(),
            affected_sources,
            confidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stewardship_finding_construction() {
        let finding = StewardshipFinding::new(
            FindingKind::Contradiction,
            "SQLite Version Conflict",
            "Doc A claims SQLite 3.35, Doc B claims SQLite 3.40",
            vec![SourceId("doc_a.md".to_string())],
            ConfidenceAssessment::new(0.92),
        );

        assert_eq!(finding.kind, FindingKind::Contradiction);
        assert_eq!(finding.affected_sources.len(), 1);
    }
}
