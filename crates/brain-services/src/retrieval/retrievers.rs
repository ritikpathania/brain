//! Concrete candidate retrievers implementing the `Retriever` trait.

use crate::retrieval::contracts::{Candidate, CandidateSet, QueryContext, Retriever};
use brain_domain::{CanonicalEntity, Normalizer};

/// Full-Text Search (FTS) candidate retriever matching query terms against entity names and aliases.
#[derive(Debug, Clone)]
pub struct FtsRetriever {
    entities: Vec<CanonicalEntity>,
}

impl FtsRetriever {
    /// Creates a new `FtsRetriever` backed by an in-memory canonical entity collection.
    pub fn new(entities: Vec<CanonicalEntity>) -> Self {
        Self { entities }
    }
}

impl Retriever for FtsRetriever {
    fn name(&self) -> &'static str {
        "FtsRetriever"
    }

    fn retrieve(&self, query: &QueryContext) -> CandidateSet {
        let mut candidate_set = CandidateSet::new();
        let normalized_query = Normalizer::normalize(&query.query_string);

        if normalized_query.is_empty() {
            return candidate_set;
        }

        for entity in &self.entities {
            let normalized_name = Normalizer::normalize(&entity.preferred_name);
            let mut match_score = 0.0f32;

            if normalized_name == normalized_query {
                match_score = 1.0;
            } else if normalized_name.contains(&normalized_query)
                || normalized_query.contains(&normalized_name)
            {
                match_score = 0.75;
            } else {
                for alias in &entity.aliases {
                    let norm_alias = Normalizer::normalize(alias);
                    if norm_alias == normalized_query {
                        match_score = 0.9;
                        break;
                    } else if norm_alias.contains(&normalized_query) {
                        match_score = 0.6;
                        break;
                    }
                }
            }

            if match_score > 0.0 {
                candidate_set.add(Candidate {
                    entity_id: entity.id,
                    preferred_name: entity.preferred_name.clone(),
                    initial_score: match_score,
                    retriever_source: self.name(),
                });
            }

            if candidate_set.candidates.len() >= query.limit {
                break;
            }
        }

        candidate_set
    }
}

/// Graph traversal candidate retriever discovering k-hop connected entity candidates.
#[derive(Debug, Clone)]
pub struct GraphRetriever {
    entities: Vec<CanonicalEntity>,
}

impl GraphRetriever {
    /// Creates a new `GraphRetriever` backed by an in-memory canonical entity collection.
    pub fn new(entities: Vec<CanonicalEntity>) -> Self {
        Self { entities }
    }
}

impl Retriever for GraphRetriever {
    fn name(&self) -> &'static str {
        "GraphRetriever"
    }

    fn retrieve(&self, query: &QueryContext) -> CandidateSet {
        let mut candidate_set = CandidateSet::new();
        let normalized_query = Normalizer::normalize(&query.query_string);

        for entity in &self.entities {
            // Find root entity matching query or target_entities filter
            let is_target = query
                .target_entities
                .as_ref()
                .map(|targets| targets.contains(&entity.id))
                .unwrap_or(false);

            let is_name_match =
                Normalizer::normalize(&entity.preferred_name).contains(&normalized_query);

            if is_target || is_name_match {
                // Add entity as graph candidate
                candidate_set.add(Candidate {
                    entity_id: entity.id,
                    preferred_name: entity.preferred_name.clone(),
                    initial_score: 0.8,
                    retriever_source: self.name(),
                });

                // Add merged historical entities as connected 1-hop graph candidates
                for &merged_id in &entity.merge_history {
                    candidate_set.add(Candidate {
                        entity_id: merged_id,
                        preferred_name: format!("{} (merged)", entity.preferred_name),
                        initial_score: 0.5,
                        retriever_source: self.name(),
                    });
                }
            }

            if candidate_set.candidates.len() >= query.limit {
                break;
            }
        }

        candidate_set
    }
}
