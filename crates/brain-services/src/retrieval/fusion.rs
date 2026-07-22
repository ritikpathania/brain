use brain_core::{
    errors::BrainError,
    retrieval::{Candidate, FusionStrategy},
};
use std::collections::HashMap;

/// Reciprocal Rank Fusion (RRF) implementation of `FusionStrategy`.
///
/// RRF combines two independent candidate lists (lexical and semantic) using the
/// formula:
/// ```text
/// score(d) = Σ_r 1 / (k + rank_r(d))
/// ```
/// where `k` is a constant that dampens the effect of high-rank outliers (default 60).
///
/// # Properties
/// - Score magnitude is bounded: max score per list ≈ `1/(k+1)`.
/// - Contributions from both lists are additive, so documents retrieved by both
///   channels score higher than documents retrieved by only one.
/// - Deterministic: ties broken by lexical rank position, then node ID for full
///   stability.
#[derive(Debug, Clone)]
pub struct RrfFusionStrategy {
    /// Smoothing constant. The original RRF paper recommends k = 60.
    pub k: f64,
}

impl Default for RrfFusionStrategy {
    fn default() -> Self {
        Self { k: 60.0 }
    }
}

impl RrfFusionStrategy {
    /// Creates a new `RrfFusionStrategy` with the default smoothing constant (k=60).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `RrfFusionStrategy` with a custom smoothing constant.
    pub fn with_k(k: f64) -> Self {
        Self { k }
    }
}

impl FusionStrategy for RrfFusionStrategy {
    fn fuse(
        &self,
        lexical: Vec<Candidate>,
        semantic: Vec<Candidate>,
    ) -> Result<Vec<Candidate>, BrainError> {
        let mut rrf_scores: HashMap<brain_domain::NodeId, f64> = HashMap::new();
        // Accumulate candidates by node id
        let mut node_map: HashMap<brain_domain::NodeId, Candidate> = HashMap::new();

        // Process lexical ranking (position 0 = best)
        for (rank_idx, candidate) in lexical.iter().enumerate() {
            let rank = (rank_idx + 1) as f64;
            *rrf_scores.entry(candidate.node.id).or_insert(0.0) += 1.0 / (self.k + rank);
            node_map
                .entry(candidate.node.id)
                .or_insert_with(|| candidate.clone());
        }

        // Process semantic ranking (position 0 = best)
        for (rank_idx, candidate) in semantic.iter().enumerate() {
            let rank = (rank_idx + 1) as f64;
            *rrf_scores.entry(candidate.node.id).or_insert(0.0) += 1.0 / (self.k + rank);
            // Merge scores if already present from lexical
            node_map
                .entry(candidate.node.id)
                .and_modify(|c| {
                    if c.semantic_score.is_none() {
                        c.semantic_score = candidate.semantic_score;
                    }
                })
                .or_insert_with(|| candidate.clone());
        }

        // Produce fused list sorted by RRF score DESC, node ID ASC as tiebreaker
        let mut fused: Vec<Candidate> = node_map.into_values().collect();
        fused.sort_by(|a, b| {
            let score_a = rrf_scores.get(&a.node.id).cloned().unwrap_or(0.0);
            let score_b = rrf_scores.get(&b.node.id).cloned().unwrap_or(0.0);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.node.id.0.cmp(&b.node.id.0))
        });

        // Annotate fusion_score and final rank on each candidate
        for (idx, candidate) in fused.iter_mut().enumerate() {
            candidate.fusion_score = rrf_scores.get(&candidate.node.id).copied();
            candidate.rank = Some(idx + 1);
        }

        Ok(fused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brain_domain::{Node, NodeId, NodeType};
    use uuid::Uuid;

    fn make_candidate(
        id: Uuid,
        label: &str,
        lexical: Option<f64>,
        semantic: Option<f64>,
    ) -> Candidate {
        let node = Node::new(NodeId(id), label.to_string(), NodeType::Concept);
        Candidate {
            node,
            lexical_score: lexical,
            semantic_score: semantic,
            fusion_score: None,
            rank: None,
        }
    }

    fn fixed_uuid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn test_rrf_union_of_both_lists() {
        // Nodes in only one list should still appear in the fused result.
        let a = fixed_uuid(1);
        let b = fixed_uuid(2);
        let c = fixed_uuid(3);

        let lexical = vec![
            make_candidate(a, "alpha", Some(1.0), None),
            make_candidate(b, "beta", Some(0.8), None),
        ];
        let semantic = vec![
            make_candidate(b, "beta", None, Some(0.9)),
            make_candidate(c, "gamma", None, Some(0.7)),
        ];

        let strategy = RrfFusionStrategy::new();
        let fused = strategy.fuse(lexical, semantic).unwrap();

        assert_eq!(fused.len(), 3, "All three distinct nodes should appear");
    }

    #[test]
    fn test_rrf_document_in_both_lists_scores_higher() {
        // Document 'b' appears in both lists at rank 1; document 'a' is rank 1 lexical only.
        // Document 'b' should score higher than 'a'.
        let a = fixed_uuid(1);
        let b = fixed_uuid(2);

        let lexical = vec![
            make_candidate(a, "alpha", Some(1.0), None),
            make_candidate(b, "beta", Some(0.5), None),
        ];
        let semantic = vec![make_candidate(b, "beta", None, Some(1.0))];

        let strategy = RrfFusionStrategy::new();
        let fused = strategy.fuse(lexical, semantic).unwrap();

        assert_eq!(fused[0].node.id.0, b, "'b' (both lists) should rank first");
        assert_eq!(fused[1].node.id.0, a, "'a' (one list) should rank second");
    }

    #[test]
    fn test_rrf_empty_inputs() {
        let strategy = RrfFusionStrategy::new();
        let fused = strategy.fuse(vec![], vec![]).unwrap();
        assert!(fused.is_empty(), "Empty inputs should produce empty output");
    }

    #[test]
    fn test_rrf_single_list_semantics() {
        // When only one list has candidates, scores should be 1/(k+rank).
        let a = fixed_uuid(1);
        let b = fixed_uuid(2);
        let strategy = RrfFusionStrategy::new();

        let lexical = vec![
            make_candidate(a, "alpha", Some(1.0), None),
            make_candidate(b, "beta", Some(0.5), None),
        ];
        let fused = strategy.fuse(lexical, vec![]).unwrap();

        // a is rank 1, so RRF score = 1/(60+1) ≈ 0.01639
        // b is rank 2, so RRF score = 1/(60+2) ≈ 0.01613
        assert_eq!(fused[0].node.id.0, a, "rank-1 lexical node should be first");
        assert!(fused[0].fusion_score.unwrap() > fused[1].fusion_score.unwrap());
    }

    #[test]
    fn test_rrf_rank_and_fusion_score_are_populated() {
        let a = fixed_uuid(1);
        let strategy = RrfFusionStrategy::new();
        let lexical = vec![make_candidate(a, "alpha", Some(1.0), None)];
        let fused = strategy.fuse(lexical, vec![]).unwrap();

        assert!(fused[0].rank.is_some(), "rank should be set");
        assert!(
            fused[0].fusion_score.is_some(),
            "fusion_score should be set"
        );
        assert_eq!(fused[0].rank.unwrap(), 1);
    }

    #[test]
    fn test_rrf_determinism_same_input() {
        // Results should be identical across two calls with the same input.
        let a = fixed_uuid(10);
        let b = fixed_uuid(20);
        let strategy = RrfFusionStrategy::new();

        let make_lists = || {
            (
                vec![
                    make_candidate(a, "alpha", Some(1.0), None),
                    make_candidate(b, "beta", Some(0.5), None),
                ],
                vec![make_candidate(a, "alpha", None, Some(0.8))],
            )
        };

        let (lex1, sem1) = make_lists();
        let (lex2, sem2) = make_lists();
        let result1 = strategy.fuse(lex1, sem1).unwrap();
        let result2 = strategy.fuse(lex2, sem2).unwrap();

        let ids1: Vec<_> = result1.iter().map(|c| c.node.id.0).collect();
        let ids2: Vec<_> = result2.iter().map(|c| c.node.id.0).collect();
        assert_eq!(ids1, ids2, "RRF must be deterministic");
    }
}
