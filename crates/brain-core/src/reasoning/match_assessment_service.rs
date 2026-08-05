//! MatchAssessmentService orchestrating ConsolidationAssessment derivation using pure MatchAssessmentPolicies.

use crate::reasoning::match_assessment_policy::MatchAssessmentPolicy;
use brain_domain::{ConsolidationAssessment, GraphMatchReport, KnowledgeCandidate};

/// Pure orchestration service for deriving ConsolidationAssessment value objects.
/// Invariants:
/// - MatchAssessmentService performs orchestration only; metric computation rules live inside MatchAssessmentPolicy.
/// - Given identical inputs, produces identical ConsolidationAssessments (determinism).
#[derive(Debug, Clone, Default)]
pub struct MatchAssessmentService;

impl MatchAssessmentService {
    /// Instantiates a new `MatchAssessmentService`.
    pub fn new() -> Self {
        Self
    }

    /// Assesses a candidate and optional match report using a `MatchAssessmentPolicy` to produce a `ConsolidationAssessment`.
    pub fn assess(
        &self,
        candidate: &KnowledgeCandidate,
        report: Option<&GraphMatchReport>,
        policy: &dyn MatchAssessmentPolicy,
    ) -> ConsolidationAssessment {
        policy.assess(candidate, report)
    }
}
