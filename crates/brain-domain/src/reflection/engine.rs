//! Pure ReflectionEngine analyzing domain context and producing StewardshipReports.

use super::finding::{FindingKind, StewardshipFinding};
use super::recommendation::{RecommendationKind, StewardshipRecommendation};
use super::report::StewardshipReport;
use crate::identifiers::SourceId;
use crate::retrieval::ConfidenceAssessment;

/// Input payload item for reflection analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeFactInput {
    /// Source document identifier.
    pub source: SourceId,
    /// Text assertion content.
    pub content: String,
}

/// Pure ReflectionEngine.
#[derive(Debug, Clone, Default)]
pub struct ReflectionEngine;

impl ReflectionEngine {
    /// Creates a new ReflectionEngine.
    pub fn new() -> Self {
        Self
    }

    /// Analyzes an array of knowledge inputs and generates a deterministic StewardshipReport.
    pub fn analyze(&self, inputs: &[KnowledgeFactInput]) -> StewardshipReport {
        let mut report = StewardshipReport::new();

        // 1. Detect Duplicate Candidates
        for i in 0..inputs.len() {
            for j in (i + 1)..inputs.len() {
                if inputs[i].content == inputs[j].content {
                    let finding = StewardshipFinding::new(
                        FindingKind::Duplication,
                        "Duplicate Factual Content",
                        format!(
                            "Identical content detected between {} and {}",
                            inputs[i].source, inputs[j].source
                        ),
                        vec![inputs[i].source.clone(), inputs[j].source.clone()],
                        ConfidenceAssessment::new(1.0),
                    );

                    let rec = StewardshipRecommendation::new(
                        finding.id,
                        RecommendationKind::Merge,
                        "Consolidate duplicate memory records to eliminate redundancy",
                    );

                    report.add_finding(finding);
                    report.add_recommendation(rec);
                }
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reflection_engine_duplicate_detection() {
        let engine = ReflectionEngine::new();
        let inputs = vec![
            KnowledgeFactInput {
                source: SourceId("file_a.md".to_string()),
                content: "Brain uses SQLite FTS5 for hybrid search.".to_string(),
            },
            KnowledgeFactInput {
                source: SourceId("file_b.md".to_string()),
                content: "Brain uses SQLite FTS5 for hybrid search.".to_string(),
            },
        ];

        let report = engine.analyze(&inputs);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].kind, FindingKind::Duplication);
        assert_eq!(report.recommendations.len(), 1);
        assert_eq!(report.recommendations[0].kind, RecommendationKind::Merge);
    }
}
