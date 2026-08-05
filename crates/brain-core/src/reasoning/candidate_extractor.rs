//! CandidateExtractorService extracting KnowledgeCandidateSets from ReasoningResults using ReflectionReports.

use brain_domain::{
    CandidateConfidence, KnowledgeCandidate, KnowledgeCandidateSet, ReasoningResult,
    ReflectionReport,
};

/// Pure domain service extracting KnowledgeCandidateSets from ReasoningResults.
///
/// Invariants:
/// - CandidateExtractorService depends strictly on ReasoningResult and ReflectionReport; it never accesses ArtifactStore or MemorySubsystem directly.
/// - CandidateExtractorService delegates finding eligibility to ReflectionReport::is_candidate_eligible.
#[derive(Debug, Clone, Default)]
pub struct CandidateExtractorService;

impl CandidateExtractorService {
    /// Instantiates a new `CandidateExtractorService`.
    pub fn new() -> Self {
        Self
    }

    /// Extracts a deduplicated `KnowledgeCandidateSet` from a `ReasoningResult` guarded by a `ReflectionReport`.
    pub fn extract_candidates(
        &self,
        result: &ReasoningResult,
        report: &ReflectionReport,
    ) -> KnowledgeCandidateSet {
        let mut candidate_set = KnowledgeCandidateSet::new();

        for finding in &result.findings {
            // Check eligibility via ReflectionReport contract
            if report.is_candidate_eligible(&finding.id) {
                // Calculate candidate confidence based on evidence backing quantity
                let confidence_val = if finding.supporting_evidence.len() >= 2 {
                    CandidateConfidence::HIGH
                } else if finding.supporting_evidence.len() == 1 {
                    CandidateConfidence::MEDIUM
                } else {
                    CandidateConfidence::LOW
                };

                let candidate = KnowledgeCandidate::new(
                    result.execution_id,
                    finding.id,
                    finding.supporting_evidence.clone(),
                    confidence_val,
                    finding.value.clone(),
                );

                candidate_set.insert(candidate);
            }
        }

        candidate_set
    }
}
