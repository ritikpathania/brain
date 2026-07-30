# Phase 4 — Sub-Project 4: Search Index Projection Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-domain` (`projection::search_index`) & `crates/brain-services` (`projection::search_index`)

---

## 1. Executive Summary & Goals

The **Search Index Projection** is a pure domain read model (`SearchIndexState`, `SearchIndexReducer`) that materializes an in-memory exact normalized lexical inverted search index over active facts and entities in $O(1)$ time:
- **Lexical Tokenization**: `SearchToken` wrapping lowercased, ASCII-punctuation-split tokens.
- **Posting Lists**: `token_to_entities` mapping tokens to active matching `KnowledgeEntityId`s, and `token_to_facts` mapping tokens to active matching `FactVersionId`s.
- **Fact Ownership & Reference Counting**: `fact_tokens` tracking token frequencies per `FactVersionId`, and `entity_token_refcounts` tracking token reference counts per `KnowledgeEntityId` for exact $O(1)$ incremental posting list pruning when facts are superseded or archived.
- **Symmetric Query API**: `search_entities(query)` and `search_facts(query)` tokenizing input queries identically to indexing and returning matched entities and facts.

Additionally, `SearchIndexReducer` implements `ProjectionStateView` and is verified via `ProjectionConformanceSuite`.

---

## 2. Architecture & Domain Purity

```text
FactEvent Stream
       │
       ▼
SearchIndexReducer (brain-domain)
       │
       ▼
SearchIndexState (Normalized Inverted Index)
```

- **Zero Subsystem Dependencies**: `brain-domain` contains zero async runtimes, heavy NLP engines, or network modules.
- **Fact Token Ownership Flow**: `FactRecorded` $\rightarrow$ `tokenize` $\rightarrow$ `fact_tokens` $\rightarrow$ `token_to_facts` $\rightarrow$ `entity_token_refcounts` $\rightarrow$ `token_to_entities`.
- **Single-Writer Safety**: All state updates are deterministic and strictly sequential per event sequence number.

---

## 3. Data Models (`crates/brain-domain/src/projection/search_index/models.rs`)

```rust
/// Strongly-typed normalized lexical search token.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SearchToken(pub String);

impl SearchToken {
    /// Tokenizes input string by lowercasing and splitting on whitespace and ASCII punctuation.
    pub fn tokenize(input: &str) -> Vec<Self> {
        input
            .to_lowercase()
            .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .filter(|s| !s.is_empty())
            .map(|s| SearchToken(s.to_string()))
            .collect()
    }
}
```

---

## 4. In-Memory Search Index State (`crates/brain-domain/src/projection/search_index/state.rs`)

```rust
/// Materialized inverted search index state for active facts and entities.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchIndexState {
    /// Inverted index from token to matching active entity IDs.
    token_to_entities: HashMap<SearchToken, HashSet<KnowledgeEntityId>>,
    /// Inverted index from token to matching active fact version IDs.
    token_to_facts: HashMap<SearchToken, HashSet<FactVersionId>>,
    /// Internal map from active FactVersionId to its subject entity ID and token frequencies.
    fact_tokens: HashMap<FactVersionId, (KnowledgeEntityId, HashMap<SearchToken, usize>)>,
    /// Internal reference counts per entity per token to prune `token_to_entities` on archival/supersession.
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

    /// Internal helper extracting lexical tokens from semantic assertion content (literal values and text).
    fn extract_fact_tokens(assertion: &SemanticAssertion) -> Vec<SearchToken> {
        let mut text_parts = Vec::new();
        match &assertion.object {
            AssertionTarget::Value(LiteralValue::String(s)) => text_parts.push(s.clone()),
            AssertionTarget::Value(LiteralValue::Integer(i)) => text_parts.push(i.to_string()),
            AssertionTarget::Value(LiteralValue::Float(f)) => text_parts.push(f.to_string()),
            AssertionTarget::Value(LiteralValue::Boolean(b)) => text_parts.push(b.to_string()),
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
            return; // Idempotent ignore during replay
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

    /// Internal helper processing FactSuperseded / FactArchived event for active facts.
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
```

---

## 5. Reducer Contract (`crates/brain-domain/src/projection/search_index/reducer.rs`)

```rust
#[derive(Debug, Clone)]
pub struct SearchIndexReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: SearchIndexState,
}

impl SearchIndexReducer {
    pub fn new(id: ProjectionId, version: ProjectionVersion) -> Self {
        Self {
            id,
            version,
            state: SearchIndexState::default(),
        }
    }
}

impl ProjectionStateView for SearchIndexReducer {
    type State = SearchIndexState;
    fn state(&self) -> &Self::State {
        &self.state
    }
}

impl ProjectionReducer for SearchIndexReducer {
    fn id(&self) -> ProjectionId { self.id.clone() }
    fn version(&self) -> ProjectionVersion { self.version }

    fn apply_event(&mut self, event: &FactEvent) -> Result<(), ProjectionError> {
        match event {
            FactEvent::FactRecorded { fact, assertion } => {
                if let Some(assert) = assertion {
                    self.state.record_fact(fact, assert);
                }
            }
            FactEvent::FactSuperseded { old_fact_id, .. } => {
                self.state.remove_active_fact(old_fact_id);
            }
            FactEvent::FactArchived { fact_id, .. } => {
                self.state.remove_active_fact(fact_id);
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.state = SearchIndexState::default();
        Ok(())
    }
}
```

---

## 6. Verification & Testing Plan

### 1. Conformance Suite Tests (`crates/brain-domain/tests/conformance_tests.rs`)
- `test_search_index_conformance`: Runs `ProjectionConformanceSuite` for `SearchIndexReducer`.

### 2. Search Index Unit & Invariant Tests (`crates/brain-domain/tests/search_index_tests.rs`)
- `test_search_index_tokenization_and_symmetric_query`
- `test_search_index_record_supersede_archive_lifecycle`
- `test_search_index_duplicate_event_idempotency`

### 3. Service Runtime Integration Tests (`crates/brain-services/tests/search_index_runtime_tests.rs`)
- `test_search_index_runtime_replay_equivalence`
- `test_search_index_mixed_event_sequence`
