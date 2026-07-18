use crate::retrieval::eval_harness::{FeatureExtractor, FeatureVector, FeatureProvider, FeatureContext, RetrievalResult, Retriever};
use brain_core::errors::BrainError;
use brain_domain::NodeId;

/// Immutable calibration weights for the linear scoring model.
/// Note: Weights are calibration parameters, not learned model parameters.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RankingWeights {
    /// Coefficient for the lexical (FTS) similarity score.
    pub lexical: f64,
    /// Coefficient for the semantic similarity score.
    pub semantic: f64,
    /// Coefficient for the recency decay feature.
    pub recency: f64,
    /// Coefficient for the combined static importance / pinned feature.
    pub importance: f64,
    /// Coefficient for the provenance confidence feature.
    pub provenance_confidence: f64,
    /// Coefficient for the graph degree feature.
    pub graph_degree: f64,
    /// Coefficient for the access frequency feature.
    pub access_frequency: f64,
    /// Coefficient for the freshness decay feature.
    pub freshness_decay: f64,
}

impl RankingWeights {
    /// Creates a baseline configuration of weights.
    pub fn baseline() -> Self {
        Self {
            lexical: 1.0,
            semantic: 1.0,
            recency: 0.0,
            importance: 0.0,
            provenance_confidence: 0.0,
            graph_degree: 0.0,
            access_frequency: 0.0,
            freshness_decay: 0.0,
        }
    }
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self::baseline()
    }
}

/// A deterministic ranker that computes a score based on a linear combination of features.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        if let Some(rec) = features.recency {
            total += rec * self.weights.recency;
        }
        if let Some(imp) = features.importance {
            total += imp * self.weights.importance;
        }
        if let Some(prov) = features.provenance_confidence {
            total += prov * self.weights.provenance_confidence;
        }
        if let Some(graph) = features.graph_degree {
            total += graph * self.weights.graph_degree;
        }
        if let Some(acc) = features.access_frequency {
            total += acc * self.weights.access_frequency;
        }
        if let Some(fresh) = features.freshness_decay {
            total += fresh * self.weights.freshness_decay;
        }
        total
    }
}

impl crate::retrieval::eval_harness::models::ScoreRanker for LinearRanker {
    fn name(&self) -> &'static str {
        "linear-ranker"
    }

    fn score(&self, features: &FeatureVector) -> f64 {
        self.score(features)
    }
}

/// A decorator retriever that wraps another retriever, extracts features, ranks candidates,
/// and returns the sorted, scored candidates.
pub struct RankingRetriever<R: Retriever> {
    underlying: R,
    ranker: LinearRanker,
    provider: Option<FeatureProvider>,
    reference_time: u64,
    decay: crate::retrieval::eval_harness::RankingDecay,
}

impl<R: Retriever> RankingRetriever<R> {
    /// Instantiates a new RankingRetriever for mock/unit tests without a database provider.
    pub fn new(underlying: R, ranker: LinearRanker) -> Self {
        Self {
            underlying,
            ranker,
            provider: None,
            reference_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            decay: crate::retrieval::eval_harness::RankingDecay::default(),
        }
    }

    /// Instantiates a new RankingRetriever with a database-backed FeatureProvider and decay configuration.
    pub fn with_provider(
        underlying: R,
        ranker: LinearRanker,
        provider: FeatureProvider,
        reference_time: u64,
        decay: crate::retrieval::eval_harness::RankingDecay,
    ) -> Self {
        Self {
            underlying,
            ranker,
            provider: Some(provider),
            reference_time,
            decay,
        }
    }
}

impl<R: Retriever> Retriever for RankingRetriever<R> {
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        let mut candidates = self.underlying.retrieve(query)?;
        if candidates.is_empty() {
            return Ok(candidates);
        }

        // 1. Batch load contexts if provider is present
        let contexts = if let Some(ref provider) = self.provider {
            let node_ids: Vec<NodeId> = candidates.iter().map(|c| c.node_id).collect();
            provider.load_contexts(&node_ids)?
        } else {
            std::collections::HashMap::new()
        };

        // 2. Initialize FeatureExtractor
        let extractor = FeatureExtractor::new(self.reference_time, self.decay);

        // 3. For each candidate, extract features and compute ranking score
        for res in &mut candidates {
            let default_ctx = FeatureContext {
                updated_at: None,
                importance: None,
                pinned: false,
                provenance_confidence: None,
                graph_degree: None,
                access_count: None,
                last_observed_at: None,
            };
            let context = contexts.get(&res.node_id).unwrap_or(&default_ctx);
            let features = extractor.extract(res, context);
            let score = self.ranker.score(&features);
            res.ranking_score = Some(score);
        }

        // 4. Explicitly sort candidates before returning so that RankingRetriever always returns ranked output
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
