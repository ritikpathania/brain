use crate::identifiers::NodeId;
use crate::retrieval::models::{RetrievedCandidate, Evidence};
use std::collections::HashMap;

/// Trait defining the contract for combining multi-source candidate lists.
pub trait CandidateFusionStrategy {
    /// Combines multiple lists of retrieved candidates into a single prioritized list.
    ///
    /// **Evidence Preservation**:
    /// Implementations must preserve all incoming evidence fragments (provenance metadata)
    /// attached to candidates, ensuring no original audit trace is discarded.
    fn fuse(&self, source_runs: &[Vec<RetrievedCandidate>]) -> Vec<RetrievedCandidate>;
}

/// Reciprocal Rank Fusion (RRF) combining source candidate ranks.
pub struct ReciprocalRankFusion {
    /// RRF constant offset parameter (default 60).
    pub k: usize,
}

impl Default for ReciprocalRankFusion {
    fn default() -> Self {
        Self { k: 60 }
    }
}

impl CandidateFusionStrategy for ReciprocalRankFusion {
    fn fuse(&self, source_runs: &[Vec<RetrievedCandidate>]) -> Vec<RetrievedCandidate> {
        let mut node_to_fused: HashMap<NodeId, RetrievedCandidate> = HashMap::new();

        for run in source_runs {
            for (rank_idx, candidate) in run.iter().enumerate() {
                let rank = rank_idx + 1;
                let rrf_score = 1.0 / (self.k + rank) as f64;

                let entry = node_to_fused.entry(candidate.node_id).or_insert_with(|| RetrievedCandidate {
                    node_id: candidate.node_id,
                    source_id: "fused",
                    local_score: 0.0,
                    explanation_fragments: Vec::new(),
                });

                entry.local_score += rrf_score;
                entry.explanation_fragments.push(Evidence::FusionContribution { rrf_rank: rank });
                entry.explanation_fragments.extend(candidate.explanation_fragments.clone());
            }
        }

        let mut fused_list: Vec<RetrievedCandidate> = node_to_fused.into_values().collect();
        fused_list.sort_by(|a, b| b.local_score.partial_cmp(&a.local_score).unwrap_or(std::cmp::Ordering::Equal));
        fused_list
    }
}
