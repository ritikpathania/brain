//! ReflectionPolicy pure strategy trait and DefaultReflectionPolicy implementation.

use brain_domain::{
    ReasoningFindingKind, ReasoningReflectionFinding, ReasoningResult, ReflectionFindingKind,
    StructuredValue,
};

/// Pure strategy interface defining critique generation rules.
/// Invariants:
/// - A ReflectionPolicy is a pure strategy object containing zero mutable execution state between invocations.
/// - Reflection identifies issues; it never prescribes repairs.
pub trait ReflectionPolicy: Send + Sync + std::fmt::Debug {
    /// Critiques a `ReasoningResult` aggregate to produce reflection findings.
    fn evaluate(&self, result: &ReasoningResult) -> Vec<ReasoningReflectionFinding>;
}

/// Default stateless reflection policy checking evidence backing and finding sufficiency.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultReflectionPolicy;

impl DefaultReflectionPolicy {
    /// Instantiates a new `DefaultReflectionPolicy`.
    pub fn new() -> Self {
        Self
    }
}

impl ReflectionPolicy for DefaultReflectionPolicy {
    fn evaluate(&self, result: &ReasoningResult) -> Vec<ReasoningReflectionFinding> {
        let mut critique_findings = Vec::new();

        for finding in &result.findings {
            // Rule 1: Check missing/empty evidence for Claims or Recommendations
            if finding.supporting_evidence.is_empty()
                && (finding.kind == ReasoningFindingKind::Claim
                    || finding.kind == ReasoningFindingKind::Recommendation)
            {
                critique_findings.push(ReasoningReflectionFinding::new(
                    ReflectionFindingKind::MissingEvidence,
                    finding.id,
                    finding.supporting_evidence.clone(),
                    StructuredValue::String("Finding lacks supporting evidence set".to_string()),
                ));
            }

            // Rule 2: Check weak support if evidence set has only 1 observation backing a high-level Conclusion
            if finding.supporting_evidence.len() == 1
                && finding.kind == ReasoningFindingKind::Conclusion
            {
                critique_findings.push(ReasoningReflectionFinding::new(
                    ReflectionFindingKind::WeakSupport,
                    finding.id,
                    finding.supporting_evidence.clone(),
                    StructuredValue::String(
                        "Conclusion supported by single evidence item".to_string(),
                    ),
                ));
            }
        }

        critique_findings
    }
}
