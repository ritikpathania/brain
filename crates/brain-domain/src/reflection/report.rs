//! Comprehensive StewardshipReport aggregate collecting observations, recommendations, and resolutions.

use super::finding::StewardshipFinding;
use super::recommendation::StewardshipRecommendation;
use super::resolution::StewardshipResolution;
use serde::{Deserialize, Serialize};

/// Comprehensive report aggregate produced by the ReflectionEngine.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StewardshipReport {
    /// List of observed stewardship findings.
    pub findings: Vec<StewardshipFinding>,
    /// Suggested action recommendations.
    pub recommendations: Vec<StewardshipRecommendation>,
    /// Applied action resolutions.
    pub resolutions: Vec<StewardshipResolution>,
}

impl StewardshipReport {
    /// Creates a new empty StewardshipReport.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a finding to the report.
    pub fn add_finding(&mut self, finding: StewardshipFinding) {
        self.findings.push(finding);
    }

    /// Adds a recommendation to the report.
    pub fn add_recommendation(&mut self, recommendation: StewardshipRecommendation) {
        self.recommendations.push(recommendation);
    }

    /// Adds a resolution to the report.
    pub fn add_resolution(&mut self, resolution: StewardshipResolution) {
        self.resolutions.push(resolution);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reflection::finding::{FindingKind, StewardshipFinding};
    use crate::retrieval::ConfidenceAssessment;

    #[test]
    fn test_stewardship_report_aggregation() {
        let mut report = StewardshipReport::new();
        let finding = StewardshipFinding::new(
            FindingKind::Duplication,
            "Duplicate Notes",
            "Identical content in note_a and note_b",
            vec![],
            ConfidenceAssessment::new(0.95),
        );

        report.add_finding(finding);
        assert_eq!(report.findings.len(), 1);
    }
}
