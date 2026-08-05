//! KnowledgeGraphMatcher trait and MatchClassifier strategy.

use brain_domain::{
    DomainError, GraphMatchQuery, GraphMatchReport, GraphSimilarityScore, MatchRelationship,
};

/// Trait defining the pure strategy for classifying relationship types from similarity scores.
pub trait MatchClassifier: Send + Sync + std::fmt::Debug {
    /// Classifies relationship type given a similarity score.
    fn classify(&self, similarity: GraphSimilarityScore) -> MatchRelationship;
}

/// Default implementation of `MatchClassifier`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultMatchClassifier;

impl DefaultMatchClassifier {
    /// Instantiates a new `DefaultMatchClassifier`.
    pub fn new() -> Self {
        Self
    }
}

impl MatchClassifier for DefaultMatchClassifier {
    fn classify(&self, similarity: GraphSimilarityScore) -> MatchRelationship {
        if similarity >= GraphSimilarityScore::EXACT {
            MatchRelationship::Duplicate
        } else if similarity >= GraphSimilarityScore::HIGH {
            MatchRelationship::Overlap
        } else if similarity >= GraphSimilarityScore::MEDIUM {
            MatchRelationship::Related
        } else {
            MatchRelationship::Related
        }
    }
}

/// Trait defining observational Knowledge Graph matching capabilities.
///
/// Invariants:
/// - Graph matching is observational only; it produces match reports but zero consolidation decisions or memory mutations.
pub trait KnowledgeGraphMatcher: Send + Sync + std::fmt::Debug {
    /// Executes observational graph matching for a candidate query.
    fn match_query(&self, query: &GraphMatchQuery) -> Result<GraphMatchReport, DomainError>;
}
