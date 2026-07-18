use crate::identifiers::NodeId;
use crate::retrieval::models::{
    Evidence, RetrievalExplanation, RetrievedCandidate, ScoredCandidate,
};
use std::collections::HashMap;

/// Trait defining the contract for normalizing and sorting fused candidate lists.
pub trait RankingStrategy {
    /// Normalizes, scores, and sorts candidates producing structured explanations.
    ///
    /// **Evidence Preservation**:
    /// Implementations must preserve all incoming evidence fragments (provenance metadata)
    /// attached to candidates, ensuring no original audit trace is discarded.
    fn rank(
        &self,
        candidates: &[RetrievedCandidate],
    ) -> (Vec<ScoredCandidate>, HashMap<NodeId, RetrievalExplanation>);
}

/// Simple strategy normalising scores by maximum value and tie-breaking by NodeId.
pub struct NormalizedTieBreakerRanking;

impl RankingStrategy for NormalizedTieBreakerRanking {
    fn rank(
        &self,
        candidates: &[RetrievedCandidate],
    ) -> (Vec<ScoredCandidate>, HashMap<NodeId, RetrievalExplanation>) {
        if candidates.is_empty() {
            return (Vec::new(), HashMap::new());
        }

        let max_score = candidates
            .iter()
            .map(|c| c.local_score)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(1.0);

        let safe_max = if max_score > 0.0 { max_score } else { 1.0 };

        let mut scored = Vec::with_capacity(candidates.len());
        let mut explanations = HashMap::with_capacity(candidates.len());

        for candidate in candidates {
            let normalized_score = candidate.local_score / safe_max;

            scored.push(ScoredCandidate {
                node_id: candidate.node_id,
                score: normalized_score,
            });

            let mut evidence_list = candidate.explanation_fragments.clone();
            evidence_list.push(Evidence::RankingAdjustment { boost: 1.0 });

            explanations.insert(candidate.node_id, RetrievalExplanation { evidence_list });
        }

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.node_id.cmp(&b.node_id))
        });

        (scored, explanations)
    }
}
