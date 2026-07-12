use crate::identifiers::NodeId;
use crate::retrieval::models::{CanonicalQuery, RetrievalExecutionContext};

/// Immutable metadata representing a predicted speculation plan.
#[derive(Debug, Clone)]
pub struct SpeculationPlan {
    /// Speculatively predicted seed nodes.
    pub predicted_seeds: Vec<NodeId>,
    /// Confidence score of this prediction.
    pub confidence: f64,
    /// Human-readable reason for the prediction.
    pub reason: String,
}

/// Interface for predicting retrieval graph expansion seeds.
pub trait SpeculationStrategy: Send + Sync {
    /// Formulates a speculative prediction of retrieval seeds prior to execution.
    fn predict(
        &self,
        query: &CanonicalQuery,
        context: &RetrievalExecutionContext,
    ) -> SpeculationPlan;
}

/// Crate-private null prediction strategy (returns empty seeds).
pub(crate) struct NoSpeculationStrategy;

impl SpeculationStrategy for NoSpeculationStrategy {
    fn predict(
        &self,
        _query: &CanonicalQuery,
        _context: &RetrievalExecutionContext,
    ) -> SpeculationPlan {
        SpeculationPlan {
            predicted_seeds: vec![],
            confidence: 0.0,
            reason: "NoSpeculationStrategy (default/null)".to_string(),
        }
    }
}

/// Heuristic prediction strategy using token substring matches against graph nodes.
pub struct SubstringSpeculationStrategy;

impl SpeculationStrategy for SubstringSpeculationStrategy {
    fn predict(
        &self,
        query: &CanonicalQuery,
        context: &RetrievalExecutionContext,
    ) -> SpeculationPlan {
        let mut predicted_seeds = Vec::new();
        let query_words: Vec<String> = query.semantic_query
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();

        if !query_words.is_empty() {
            for (&id, node) in &context.graph.nodes {
                let label_lower = node.label.to_lowercase();
                if query_words.iter().any(|word| label_lower.contains(word)) {
                    predicted_seeds.push(id);
                }
            }
        }

        SpeculationPlan {
            predicted_seeds,
            confidence: 0.8,
            reason: "SubstringSpeculationStrategy matches".to_string(),
        }
    }
}
