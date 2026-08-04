//! Stateless, pure SynthesisPolicy trait and DefaultSynthesisPolicy implementation.

use brain_domain::{ArtifactView, EvidenceSet, ReasoningFinding, ReasoningFindingKind};

/// Pure strategy interface decoupling synthesis interpretation rules from service execution.
/// Invariant: A SynthesisPolicy is a pure strategy object with zero mutable execution state between invocations.
pub trait SynthesisPolicy: Send + Sync + std::fmt::Debug {
    /// Filters and synthesizes domain findings from resolved evidence views.
    fn interpret(
        &self,
        evidence_set: &EvidenceSet,
        views: &[ArtifactView<'_>],
    ) -> Vec<ReasoningFinding>;
}

/// Default stateless synthesis policy mapping artifact views into domain findings.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultSynthesisPolicy;

impl DefaultSynthesisPolicy {
    /// Instantiates a new `DefaultSynthesisPolicy`.
    pub fn new() -> Self {
        Self
    }
}

impl SynthesisPolicy for DefaultSynthesisPolicy {
    fn interpret(
        &self,
        evidence_set: &EvidenceSet,
        views: &[ArtifactView<'_>],
    ) -> Vec<ReasoningFinding> {
        views
            .iter()
            .map(|view| {
                let kind = match view.metadata().kind {
                    brain_domain::EvidenceArtifactKind::RawData => ReasoningFindingKind::Observation,
                    brain_domain::EvidenceArtifactKind::DerivedData => ReasoningFindingKind::Claim,
                    brain_domain::EvidenceArtifactKind::Claim => ReasoningFindingKind::Claim,
                    brain_domain::EvidenceArtifactKind::Summary => ReasoningFindingKind::Conclusion,
                    brain_domain::EvidenceArtifactKind::Result => ReasoningFindingKind::Recommendation,
                };
                ReasoningFinding::new(kind, view.value().clone(), evidence_set.clone())
            })
            .collect()
    }
}
