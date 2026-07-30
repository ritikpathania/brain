//! In-memory inverted search index state.

use crate::bkf::*;
use crate::projection::search_index::models::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Materialized inverted search index state for active facts and entities.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchIndexState {
    token_to_entities: HashMap<SearchToken, HashSet<KnowledgeEntityId>>,
    token_to_facts: HashMap<SearchToken, HashSet<FactVersionId>>,
    fact_tokens: HashMap<FactVersionId, (KnowledgeEntityId, HashMap<SearchToken, usize>)>,
    entity_token_refcounts: HashMap<KnowledgeEntityId, HashMap<SearchToken, usize>>,
}

impl SearchIndexState {
    /// Symmetric search returning active entity IDs matching any token in the query string.
    pub fn search_entities(&self, query: &str) -> HashSet<KnowledgeEntityId> {
        let tokens = SearchToken::tokenize(query);
        let mut results = HashSet::new();
        for token in tokens {
            if let Some(entities) = self.token_to_entities.get(&token) {
                results.extend(entities.iter().cloned());
            }
        }
        results
    }

    /// Symmetric search returning active fact version IDs matching any token in the query string.
    pub fn search_facts(&self, query: &str) -> HashSet<FactVersionId> {
        let tokens = SearchToken::tokenize(query);
        let mut results = HashSet::new();
        for token in tokens {
            if let Some(facts) = self.token_to_facts.get(&token) {
                results.extend(facts.iter().cloned());
            }
        }
        results
    }

    /// Returns total count of indexed unique tokens.
    pub fn len(&self) -> usize {
        self.token_to_facts.len()
    }

    /// Returns true if no tokens are indexed.
    pub fn is_empty(&self) -> bool {
        self.token_to_facts.is_empty()
    }

    fn extract_fact_tokens(assertion: &SemanticAssertion) -> Vec<SearchToken> {
        let mut text_parts = Vec::new();
        match &assertion.object {
            AssertionTarget::Value(LiteralValue::String(s)) => text_parts.push(s.clone()),
            AssertionTarget::Value(LiteralValue::Integer(i)) => text_parts.push(i.to_string()),
            AssertionTarget::Value(LiteralValue::Float(f)) => text_parts.push(f.to_string()),
            AssertionTarget::Value(LiteralValue::Boolean(b)) => text_parts.push(b.to_string()),
            AssertionTarget::Value(LiteralValue::Timestamp(_)) => {}
            AssertionTarget::Entity(e) => text_parts.push(e.0.to_string()),
        }

        text_parts
            .iter()
            .flat_map(|part| SearchToken::tokenize(part))
            .collect()
    }

    /// Internal helper processing FactRecorded event. Idempotent on duplicate `fact.id`.
    pub fn record_fact(&mut self, fact: &FactVersion, assertion: &SemanticAssertion) {
        if self.fact_tokens.contains_key(&fact.id) {
            return;
        }

        let entity_id = assertion.subject.clone();
        let tokens = Self::extract_fact_tokens(assertion);

        let mut token_counts: HashMap<SearchToken, usize> = HashMap::new();
        for token in tokens {
            *token_counts.entry(token).or_default() += 1;
        }

        for (token, _count) in &token_counts {
            self.token_to_facts
                .entry(token.clone())
                .or_default()
                .insert(fact.id.clone());
        }

        let entity_refcounts = self.entity_token_refcounts.entry(entity_id.clone()).or_default();
        for (token, _count) in &token_counts {
            let refcount = entity_refcounts.entry(token.clone()).or_default();
            let is_new = *refcount == 0;
            *refcount += 1;
            if is_new {
                self.token_to_entities
                    .entry(token.clone())
                    .or_default()
                    .insert(entity_id.clone());
            }
        }

        self.fact_tokens.insert(fact.id.clone(), (entity_id, token_counts));
    }

    /// Internal helper processing FactSuperseded / FactArchived event.
    pub fn remove_active_fact(&mut self, fact_id: &FactVersionId) {
        if let Some((entity_id, token_counts)) = self.fact_tokens.remove(fact_id) {
            for (token, _count) in &token_counts {
                if let Some(fact_set) = self.token_to_facts.get_mut(token) {
                    fact_set.remove(fact_id);
                    if fact_set.is_empty() {
                        self.token_to_facts.remove(token);
                    }
                }
            }

            let mut remove_entity = false;
            if let Some(entity_refcounts) = self.entity_token_refcounts.get_mut(&entity_id) {
                for (token, _count) in &token_counts {
                    if let Some(cnt) = entity_refcounts.get_mut(token) {
                        *cnt = cnt.saturating_sub(1);
                        if *cnt == 0 {
                            entity_refcounts.remove(token);
                            if let Some(entity_set) = self.token_to_entities.get_mut(token) {
                                entity_set.remove(&entity_id);
                                if entity_set.is_empty() {
                                    self.token_to_entities.remove(token);
                                }
                            }
                        }
                    }
                }
                if entity_refcounts.is_empty() {
                    remove_entity = true;
                }
            }

            if remove_entity {
                self.entity_token_refcounts.remove(&entity_id);
            }
        }
    }
}
