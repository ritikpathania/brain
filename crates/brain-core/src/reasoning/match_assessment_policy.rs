//! MatchAssessmentPolicy pure strategy trait and DefaultMatchAssessmentPolicy implementation.

use brain_domain::{
    AssessmentExplanation, AssessmentMetrics, ConsolidationAssessment, ContradictionProbability,
    DuplicateProbability, GraphMatchReport, KnowledgeCandidate, MatchRelationship,
};

/// Pure strategy interface defining assessment metric derivation rules from match reports.
/// Invariants:
/// - A MatchAssessmentPolicy is a pure strategy object containing zero mutable execution state between invocations.
/// - Derives metrics but zero consolidation decisions.
pub trait MatchAssessmentPolicy: Send + Sync + std::fmt::Debug {
    /// Assesses candidate and match report to produce a derived `ConsolidationAssessment`.
    fn assess(
        &self,
        candidate: &KnowledgeCandidate,
        report: Option<&GraphMatchReport>,
    ) -> ConsolidationAssessment;
}

/// Default stateless match assessment policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultMatchAssessmentPolicy;

impl DefaultMatchAssessmentPolicy {
    /// Instantiates a new `DefaultMatchAssessmentPolicy`.
    pub fn new() -> Self {
        Self
    }
}

impl MatchAssessmentPolicy for DefaultMatchAssessmentPolicy {
    fn assess(
        &self,
        candidate: &KnowledgeCandidate,
        report: Option<&GraphMatchReport>,
    ) -> ConsolidationAssessment {
        let (dup_prob, contra_prob) = match report {
            Some(rep) => {
                let mut max_dup = DuplicateProbability::NONE;
                let mut max_contra = ContradictionProbability::NONE;

                for match_item in rep.iter() {
                    match match_item.relationship {
                        MatchRelationship::Duplicate => {
                            if match_item.similarity.value() >= 0.9 {
                                max_dup = DuplicateProbability::CERTAIN;
                            } else {
                                max_dup = DuplicateProbability::HIGH;
                            }
                        }
                        MatchRelationship::Contradiction => {
                            max_contra = ContradictionProbability::HIGH;
                        }
                        MatchRelationship::Overlap => {
                            if max_dup < DuplicateProbability::MODERATE {
                                max_dup = DuplicateProbability::MODERATE;
                            }
                        }
                        MatchRelationship::Related => {}
                    }
                }
                (max_dup, max_contra)
            }
            None => (DuplicateProbability::NONE, ContradictionProbability::NONE),
        };

        let metrics = AssessmentMetrics::new(dup_prob, contra_prob);
        let explanation = match report {
            Some(rep) => AssessmentExplanation::with_report(rep.clone()),
            None => AssessmentExplanation::empty(),
        };

        ConsolidationAssessment::new(candidate.confidence, metrics, explanation)
    }
}
