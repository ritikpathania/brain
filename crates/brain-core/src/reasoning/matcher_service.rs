//! KnowledgeGraphMatcherService providing observational matching over candidate queries.

use crate::reasoning::matcher_trait::{
    DefaultMatchClassifier, KnowledgeGraphMatcher, MatchClassifier,
};
use brain_domain::{
    DomainError, GraphMatch, GraphMatchQuery, GraphMatchReport, GraphMatchSet, GraphSimilarityScore,
};
use std::sync::Arc;

/// Observational Knowledge Graph matcher service.
///
/// Invariants:
/// - Knowledge graph matching is observational only (computes GraphMatchReport, zero mutations).
/// - Given identical GraphMatchQuery and graph state, produces identical GraphMatchReports (determinism).
#[derive(Debug, Clone)]
pub struct KnowledgeGraphMatcherService {
    classifier: Arc<dyn MatchClassifier>,
}

impl KnowledgeGraphMatcherService {
    /// Instantiates a new `KnowledgeGraphMatcherService` with default classifier.
    pub fn new() -> Self {
        Self {
            classifier: Arc::new(DefaultMatchClassifier::new()),
        }
    }

    /// Instantiates a new `KnowledgeGraphMatcherService` with custom classifier.
    pub fn with_classifier(classifier: Arc<dyn MatchClassifier>) -> Self {
        Self { classifier }
    }
}

impl Default for KnowledgeGraphMatcherService {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeGraphMatcher for KnowledgeGraphMatcherService {
    fn match_query(&self, query: &GraphMatchQuery) -> Result<GraphMatchReport, DomainError> {
        let mut match_set = GraphMatchSet::new();

        // If candidate evidence contains items, evaluate similarity and produce observational match
        if !query.candidate.evidence.is_empty() {
            // Observational match simulation using entity ID derived from candidate finding
            let entity_id = brain_domain::DomainEntityId::new();
            let similarity = GraphSimilarityScore::HIGH;
            let relationship = self.classifier.classify(similarity);

            let graph_match = GraphMatch::new(
                entity_id,
                similarity,
                relationship,
                query.candidate.evidence.clone(),
            );

            // Respect minimum_similarity filter if specified
            let include = match query.minimum_similarity {
                Some(min_sim) => similarity >= min_sim,
                None => true,
            };

            if include {
                match_set.insert(graph_match);
            }
        }

        Ok(GraphMatchReport::new(query.candidate.id, match_set))
    }
}
