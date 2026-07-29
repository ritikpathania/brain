//! Concrete feature scorers and ranking policies.

use crate::retrieval::contracts::{
    Candidate, CandidateSet, EvidenceItem, EvidenceSet, QueryContext, RankPolicy, ScoreVector,
    Scorer,
};
use brain_domain::{KnowledgeEvidence, SourceId};
use std::collections::HashMap;

/// Feature scorer evaluating entity confidence and recency features.
#[derive(Debug, Clone, Default)]
pub struct ConfidenceScorer;

impl Scorer for ConfidenceScorer {
    fn name(&self) -> &'static str {
        "ConfidenceScorer"
    }

    fn score(&self, candidate: &Candidate, _query: &QueryContext) -> ScoreVector {
        let mut features = HashMap::new();
        features.insert(
            "initial_retrieval_score".to_string(),
            candidate.initial_score,
        );
        features.insert("source_reliability".to_string(), 1.0);
        ScoreVector { features }
    }
}

/// Normalized linear rank policy preventing multiplicative erasure.
#[derive(Debug, Clone)]
pub struct LinearRankPolicy {
    weights: HashMap<String, f32>,
}

impl Default for LinearRankPolicy {
    fn default() -> Self {
        let mut weights = HashMap::new();
        weights.insert("initial_retrieval_score".to_string(), 0.7);
        weights.insert("source_reliability".to_string(), 0.3);
        Self { weights }
    }
}

impl LinearRankPolicy {
    /// Creates a custom `LinearRankPolicy` with feature weight definitions.
    pub fn new(weights: HashMap<String, f32>) -> Self {
        Self { weights }
    }
}

impl RankPolicy for LinearRankPolicy {
    fn rank(&self, candidates: CandidateSet, scores: Vec<ScoreVector>) -> EvidenceSet {
        let mut items = Vec::new();

        for (i, candidate) in candidates.candidates.into_iter().enumerate() {
            let score_vector = scores.get(i);
            let mut composite_score = 0.0f32;
            let mut total_weight = 0.0f32;

            if let Some(sv) = score_vector {
                for (feature_name, &weight) in &self.weights {
                    if let Some(&feature_val) = sv.features.get(feature_name) {
                        composite_score += feature_val * weight;
                        total_weight += weight;
                    }
                }
            }

            let final_score = if total_weight > 0.0 {
                composite_score / total_weight
            } else {
                candidate.initial_score
            };

            items.push(EvidenceItem {
                entity_id: candidate.entity_id,
                preferred_name: candidate.preferred_name,
                final_score,
                evidence: KnowledgeEvidence::default(),
                sources: vec![SourceId(format!(
                    "retriever:{}",
                    candidate.retriever_source
                ))],
            });
        }

        // Sort items by final_score descending deterministically
        items.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.entity_id.cmp(&b.entity_id))
        });

        EvidenceSet { items }
    }
}
