use crate::retrieval::eval_harness::{FeatureExtractor, FeatureVector, RetrievalResult, Retriever};
use brain_core::errors::BrainError;

/// Immutable calibration weights for the linear scoring model.
/// Note: Weights are calibration parameters, not learned model parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankingWeights {
    /// Coefficient for the lexical (FTS) similarity score.
    pub lexical: f64,
    /// Coefficient for the semantic similarity score.
    pub semantic: f64,
}

impl RankingWeights {
    /// Creates a baseline configuration of weights.
    pub fn baseline() -> Self {
        Self {
            lexical: 1.0,
            semantic: 1.0,
        }
    }
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self::baseline()
    }
}

/// A deterministic ranker that computes a score based on a linear combination of features.
pub struct LinearRanker {
    weights: RankingWeights,
}

impl LinearRanker {
    /// Instantiates a new LinearRanker with the given configuration weights.
    pub fn new(weights: RankingWeights) -> Self {
        Self { weights }
    }

    /// Evaluates the feature vector to compute a single ranking score.
    pub fn score(&self, features: &FeatureVector) -> f64 {
        let mut total = 0.0;
        if let Some(lex) = features.lexical_similarity {
            total += lex * self.weights.lexical;
        }
        if let Some(sem) = features.semantic_similarity {
            total += sem * self.weights.semantic;
        }
        total
    }
}

/// A decorator retriever that wraps another retriever, extracts features, ranks candidates,
/// and returns the sorted, scored candidates.
pub struct RankingRetriever<R: Retriever> {
    underlying: R,
    ranker: LinearRanker,
}

impl<R: Retriever> RankingRetriever<R> {
    /// Instantiates a new RankingRetriever wrapping an underlying candidate generator.
    pub fn new(underlying: R, ranker: LinearRanker) -> Self {
        Self { underlying, ranker }
    }
}

impl<R: Retriever> Retriever for RankingRetriever<R> {
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        let mut candidates = self.underlying.retrieve(query)?;
        for res in &mut candidates {
            let features = FeatureExtractor::extract(res);
            let score = self.ranker.score(&features);
            res.ranking_score = Some(score);
        }
        // Explicitly sort candidates before returning so that RankingRetriever always returns ranked output
        super::sort_results_deterministically(&mut candidates);
        Ok(candidates)
    }

    fn normalize_query(&self, query: &str) -> Option<String> {
        self.underlying.normalize_query(query)
    }

    fn executed_query(&self, query: &str) -> Option<String> {
        self.underlying.executed_query(query)
    }
}
