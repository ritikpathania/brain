//! Pluggable candidate fusion engine and Reciprocal Rank Fusion (RRF) implementation.

use crate::compiler::EntityId;
use crate::query::executor::{Candidate, RawCandidateSet};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Final merged and ranked query result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    /// Ranked candidate list ordered by fused relevance score.
    pub candidates: Vec<Candidate>,
    /// Total count of unique candidate entities discovered before limit truncation.
    pub total_candidates: usize,
}

/// Trait defining a candidate set fusion and re-ranking strategy.
pub trait FusionStrategy: Send + Sync {
    /// Fuses multiple raw candidate sets into a unified, ranked `QueryResult`.
    fn fuse(&self, candidate_sets: &[RawCandidateSet], limit: usize) -> QueryResult;
}

/// Standard Reciprocal Rank Fusion (RRF) implementation.
#[derive(Debug, Clone)]
pub struct ReciprocalRankFusion {
    k: f32,
}

impl Default for ReciprocalRankFusion {
    fn default() -> Self {
        Self { k: 60.0 }
    }
}

impl ReciprocalRankFusion {
    /// Instantiates a new `ReciprocalRankFusion` strategy with standard default rank constant k=60.0.
    pub fn new() -> Self {
        Self::default()
    }
}

impl FusionStrategy for ReciprocalRankFusion {
    fn fuse(&self, candidate_sets: &[RawCandidateSet], limit: usize) -> QueryResult {
        let mut rrf_scores: HashMap<EntityId, f32> = HashMap::new();

        for set in candidate_sets {
            // Rank candidates by score descending
            let mut sorted = set.candidates.clone();
            sorted.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            for (rank_idx, candidate) in sorted.iter().enumerate() {
                let rank = (rank_idx + 1) as f32;
                let rrf_score = 1.0 / (self.k + rank);
                *rrf_scores.entry(candidate.entity_id.clone()).or_insert(0.0) += rrf_score;
            }
        }

        let total_candidates = rrf_scores.len();

        let mut ranked: Vec<Candidate> = rrf_scores
            .into_iter()
            .map(|(entity_id, score)| Candidate { entity_id, score })
            .collect();

        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if limit > 0 && ranked.len() > limit {
            ranked.truncate(limit);
        }

        QueryResult {
            candidates: ranked,
            total_candidates,
        }
    }
}
