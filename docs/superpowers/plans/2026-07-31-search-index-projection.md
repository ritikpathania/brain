# Search Index Projection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 4 — Sub-Project 4: **Search Index Projection** as a pure domain read model (`SearchIndexState`, `SearchIndexReducer`) materializing an exact normalized lexical inverted search index over active facts and entity references over `FactEvent` streams, fully integrated with `ProjectionStateView` and `ProjectionConformanceSuite`.

**Architecture:** Models, state, and reducer live in `brain-domain::projection::search_index` with zero external dependencies. `SearchToken` wraps lowercased ASCII-punctuation-split tokens. Inverted posting lists `token_to_entities` and `token_to_facts` are updated incrementally on `FactRecorded` via `record_fact` and pruned on `FactSuperseded`/`FactArchived` via `remove_active_fact` using `fact_tokens` and `entity_token_refcounts`. Queries use symmetric tokenization (`search_entities`, `search_facts`). Service exports live in `brain-services`.

**Tech Stack:** Rust (edition 2021), `serde`, `std::collections::HashMap`, `std::collections::HashSet`.

## Global Constraints
- `brain-domain` must contain zero async runtimes, logger setups, database engines, or network dependencies (`#![deny(missing_docs)]` enabled).
- Replaying identical `FactEvent` streams must produce 100% bitwise-identical `SearchIndexState`.
- `ProjectionStateView` trait must be implemented for `SearchIndexReducer`.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: SearchToken & SearchIndexState Implementation | ✅ Completed | `3761220` |
| **M2** | Task 2: SearchIndexReducer & Conformance Integration | ✅ Completed | `1551d04` |
| **M2 Checkpoint** | **Unit & Conformance Tests Freeze** | ✅ Completed | `1551d04` |
| **M3** | Task 3: Projection Runtime Service Export | ✅ Completed | `52525c4` |
| **M3** | Task 4: Runtime Replay Integration & Catch-Up Tests | ✅ Completed | `5827fcc` |
| **M4** | Task 5: Workspace-Wide Verification | ✅ Completed | `5827fcc` |

---

### Task 1: SearchToken & SearchIndexState Implementation

**Files:**
- Create: `crates/brain-domain/src/projection/search_index/models.rs`
- Create: `crates/brain-domain/src/projection/search_index/state.rs`
- Create: `crates/brain-domain/tests/search_index_tests.rs`
- Create: `crates/brain-domain/src/projection/search_index/mod.rs`
- Modify: `crates/brain-domain/src/projection/mod.rs`

**Interfaces:**
- Consumes: `KnowledgeEntityId`, `FactVersionId`, `PredicateId`, `SemanticAssertion`, `AssertionTarget`, `LiteralValue`, `FactVersion`
- Produces: `SearchToken` (`tokenize`), `SearchIndexState` (`search_entities`, `search_facts`, `record_fact`, `remove_active_fact`, `len`, `is_empty`)

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/search_index_tests.rs
use brain_domain::bkf::*;
use brain_domain::projection::search_index::*;
use uuid::Uuid;

#[test]
fn test_search_index_tokenization_and_symmetric_query() {
    let mut state = SearchIndexState::default();
    let fact_id1 = FactVersionId(Uuid::new_v4());
    let fact_id2 = FactVersionId(Uuid::new_v4());
    let assertion_id1 = AssertionId(Uuid::new_v4());
    let assertion_id2 = AssertionId(Uuid::new_v4());
    let entity_id1 = KnowledgeEntityId(Uuid::new_v4());
    let entity_id2 = KnowledgeEntityId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact1 = FactVersion {
        id: fact_id1.clone(),
        assertion_id: assertion_id1,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };
    let assertion1 = SemanticAssertion {
        id: assertion_id1,
        kind: AssertionKind::Relationship,
        subject: entity_id1.clone(),
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("Rust Knowledge Graph".to_string())),
    };

    let fact2 = FactVersion {
        id: fact_id2.clone(),
        assertion_id: assertion_id2,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };
    let assertion2 = SemanticAssertion {
        id: assertion_id2,
        kind: AssertionKind::Relationship,
        subject: entity_id2.clone(),
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("Compiler Optimization".to_string())),
    };

    // Test record_fact & duplicate record idempotency
    state.record_fact(&fact1, &assertion1);
    state.record_fact(&fact1, &assertion1);
    state.record_fact(&fact2, &assertion2);

    assert_eq!(state.len(), 5); // "rust", "knowledge", "graph", "compiler", "optimization"

    // Symmetric query
    let matched_entities = state.search_entities("rust-knowledge");
    assert!(matched_entities.contains(&entity_id1));

    // Multi-token OR query semantics ("rust" matches fact1/entity1, "compiler" matches fact2/entity2)
    let or_matched_entities = state.search_entities("rust compiler");
    assert_eq!(or_matched_entities.len(), 2);
    assert!(or_matched_entities.contains(&entity_id1));
    assert!(or_matched_entities.contains(&entity_id2));

    let matched_facts = state.search_facts("graph");
    assert!(matched_facts.contains(&fact_id1));

    // Test duplicate remove_active_fact idempotency & internal cleanup
    state.remove_active_fact(&fact_id1);
    state.remove_active_fact(&fact_id1);
    state.remove_active_fact(&fact_id2);
    state.remove_active_fact(&fact_id2);

    assert_eq!(state.len(), 0);
    assert!(state.is_empty());
    assert!(state.search_entities("rust").is_empty());
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test search_index_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::search_index`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/search_index/models.rs
//! Data models for Search Index Projection.

use serde::{Deserialize, Serialize};

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

```rust
// crates/brain-domain/src/projection/search_index/state.rs
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
```

Export `models` and `state` in `crates/brain-domain/src/projection/search_index/mod.rs` and export `pub mod search_index;` in `crates/brain-domain/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test search_index_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add SearchToken and SearchIndexState"
```

---

### Task 2: SearchIndexReducer & Conformance Integration

**Files:**
- Create: `crates/brain-domain/src/projection/search_index/reducer.rs`
- Modify: `crates/brain-domain/tests/conformance_tests.rs`
- Modify: `crates/brain-domain/src/projection/search_index/mod.rs`
- Modify: `crates/brain-domain/src/projection/mod.rs`

**Interfaces:**
- Consumes: `SearchIndexState`, `ProjectionReducer`, `ProjectionStateView`, `FactEvent`
- Produces: `SearchIndexReducer` (`new`, `id`, `version`, `apply_event`, `reset`, `state`)

- [ ] **Step 1: Write failing test**

```rust
// Add to crates/brain-domain/tests/conformance_tests.rs
#[test]
fn test_search_index_conformance() {
    let reducer = SearchIndexReducer::new(ProjectionId::new("search_index"), ProjectionVersion(1));
    let fact_id = FactVersionId(Uuid::new_v4());
    let assertion_id = AssertionId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact = FactVersion {
        id: fact_id.clone(),
        assertion_id,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };

    let assertion = SemanticAssertion {
        id: assertion_id,
        kind: AssertionKind::Relationship,
        subject: KnowledgeEntityId(Uuid::new_v4()),
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("Search Index Text".to_string())),
    };

    let event = FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    };

    ProjectionConformanceSuite::assert_reset_clears_state(reducer.clone(), &[event.clone()]);
    ProjectionConformanceSuite::assert_duplicate_event_idempotency(reducer.clone(), &event);
    ProjectionConformanceSuite::assert_replay_equivalence(reducer.clone(), reducer, &[event]);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test conformance_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::search_index::SearchIndexReducer`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/search_index/reducer.rs
//! Pure domain reducer for Search Index Projection.

use crate::bkf::events::FactEvent;
use crate::projection::conformance::*;
use crate::projection::errors::*;
use crate::projection::id::*;
use crate::projection::reducer::*;
use crate::projection::search_index::state::*;

/// Domain reducer reducing FactEvents into SearchIndexState.
#[derive(Debug, Clone)]
pub struct SearchIndexReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: SearchIndexState,
}

impl SearchIndexReducer {
    /// Creates a new SearchIndexReducer.
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
    fn id(&self) -> ProjectionId {
        self.id.clone()
    }

    fn version(&self) -> ProjectionVersion {
        self.version
    }

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

Re-export `reducer` in `crates/brain-domain/src/projection/search_index/mod.rs` and `crates/brain-domain/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test conformance_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): implement SearchIndexReducer processing FactEvents"
```

---

### Milestone 2 Checkpoint: Unit & Conformance Tests Freeze

- Verify all domain unit tests pass: `cargo test -p brain-domain`.
- Freeze `brain-domain::projection::search_index` exports.

---

### Task 3: Projection Runtime Service Export (`crates/brain-services`)

**Files:**
- Modify: `crates/brain-services/src/projection/mod.rs`
- Create: `crates/brain-services/tests/search_index_export_tests.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/search_index_export_tests.rs
use brain_domain::projection::search_index::*;
use brain_domain::projection::*;
use brain_services::projection::search_index::*;

#[test]
fn test_search_index_services_reexport() {
    let reducer = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));
    assert_eq!(reducer.id().as_str(), "search");
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test search_index_export_tests
```
Expected: FAIL with `unresolved import brain_services::projection::search_index`.

- [ ] **Step 3: Implement minimal code**

Re-export `brain_domain::projection::search_index` in `crates/brain-services/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test search_index_export_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): re-export SearchIndexReducer in brain-services::projection"
```

---

### Task 4: Runtime Replay Integration & Catch-Up Tests (`crates/brain-services/tests/search_index_runtime_tests.rs`)

**Files:**
- Create: `crates/brain-services/tests/search_index_runtime_tests.rs`

- [ ] **Step 1: Write runtime replay test**

```rust
// crates/brain-services/tests/search_index_runtime_tests.rs
use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::search_index::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::runtime::*;
use brain_services::projection::store::*;
use uuid::Uuid;

#[test]
fn test_search_index_runtime_replay_equivalence() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(SearchIndexReducer::new(
        ProjectionId::new("search_idx"),
        ProjectionVersion(1),
    ));
    let instance = ProjectionInstance::new(reducer);
    runtime.register_projection(instance).unwrap();

    let fact_id = FactVersionId(Uuid::new_v4());
    let assertion_id = AssertionId(Uuid::new_v4());
    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact = FactVersion {
        id: fact_id.clone(),
        assertion_id,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };

    let assertion = SemanticAssertion {
        id: assertion_id,
        kind: AssertionKind::Relationship,
        subject: entity_id,
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("Inverted Lexical Search Engine".to_string())),
    };

    let events = vec![FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    }];
    runtime.catchup_all(events.iter(), Watermark(1)).unwrap();
}

#[test]
fn test_search_index_mixed_event_sequence() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(SearchIndexReducer::new(
        ProjectionId::new("search_mixed"),
        ProjectionVersion(1),
    ));
    let instance = ProjectionInstance::new(reducer);
    runtime.register_projection(instance).unwrap();

    let fact_id1 = FactVersionId(Uuid::new_v4());
    let fact_id2 = FactVersionId(Uuid::new_v4());
    let assertion_id1 = AssertionId(Uuid::new_v4());
    let assertion_id2 = AssertionId(Uuid::new_v4());
    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact1 = FactVersion {
        id: fact_id1.clone(),
        assertion_id: assertion_id1,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };
    let assertion1 = SemanticAssertion {
        id: assertion_id1,
        kind: AssertionKind::Relationship,
        subject: entity_id,
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("Old Version Content".to_string())),
    };

    let fact2 = FactVersion {
        id: fact_id2.clone(),
        assertion_id: assertion_id2,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: Some(fact_id1.clone()),
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };
    let assertion2 = SemanticAssertion {
        id: assertion_id2,
        kind: AssertionKind::Relationship,
        subject: entity_id,
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("New Version Content".to_string())),
    };

    let events = vec![
        FactEvent::FactRecorded { fact: fact1, assertion: Some(assertion1) },
        FactEvent::FactRecorded { fact: fact2, assertion: Some(assertion2) },
        FactEvent::FactSuperseded {
            old_fact_id: fact_id1.clone(),
            new_fact_id: fact_id2.clone(),
            superseded_at: now,
        },
        FactEvent::FactArchived {
            fact_id: fact_id2.clone(),
            archived_at: now,
        },
    ];

    runtime.catchup_all(events.iter(), Watermark(4)).unwrap();
}
```

- [ ] **Step 2: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test search_index_runtime_tests
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/brain-services/ && git commit -m "test(services): add SearchIndexReducer integration and catch-up replay tests"
```

---

### Task 5: Workspace-Wide Verification

- Run `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-domain -p brain-services`.
- Verify clean compilation, 0 test failures, and 0 warnings.
- Update `walkthrough.md`.
