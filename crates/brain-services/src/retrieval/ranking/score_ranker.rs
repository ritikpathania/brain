//! Score ranker module.

use crate::retrieval::ranking::feature_provider::FeatureVector;

/// First-class runtime scoring contract for machine learned models.
pub trait ScoreRanker: Send + Sync {
    /// Returns the name identifier of the ranker model.
    fn name(&self) -> &'static str;
    /// Predicts/scores the feature vector.
    fn score(&self, features: &FeatureVector) -> f64;
}
