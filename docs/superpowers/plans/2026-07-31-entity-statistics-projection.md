# Entity Statistics Projection & Conformance Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 4 — Sub-Project 3: **Entity Statistics Projection** as a pure domain read model (`EntityStatisticsState`, `EntityStatisticsReducer`) materializing operational metrics per entity over `FactEvent` streams, and introduce the reusable **`ProjectionConformanceSuite`** test harness and `ProjectionStateView` trait.

**Architecture:** Conformance harness lives in `brain-domain::projection::conformance`. Statistics models, normalized state maps, and reducer live in `brain-domain::projection::entity_statistics` with zero external dependencies. State lookup is encapsulated via domain read methods (`get`, `len`, `is_empty`). Reducer mutations use atomic helper functions (`record_fact`, `supersede_fact`, `archive_fact`) maintaining internal `ActiveFactMetadata` and `predicate_refcounts` for exact $O(1)$ metric tracking. Runtime integration tests live in `brain-services`.

**Tech Stack:** Rust (edition 2021), `serde`, `std::collections::HashMap`.

## Global Constraints
- `brain-domain` must contain zero async runtimes, logger setups, database engines, or network dependencies (`#![deny(missing_docs)]` enabled).
- Replaying identical `FactEvent` streams must produce 100% bitwise-identical `EntityStatisticsState`.
- `ProjectionStateView` trait must be implemented for `GraphAdjacencyReducer`, `TemporalStateReducer`, and `EntityStatisticsReducer`.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: ProjectionStateView Trait & ProjectionConformanceSuite | ⬜ Pending | |
| **M2** | Task 2: EntityStatistics & EntityStatisticsState | ⬜ Pending | |
| **M2** | Task 3: EntityStatisticsReducer Implementation | ⬜ Pending | |
| **M2 Checkpoint** | **Unit & Conformance Tests Freeze** | ⬜ Pending | |
| **M3** | Task 4: Projection Runtime Service Export | ⬜ Pending | |
| **M3** | Task 5: Runtime Replay Integration & Catch-Up Tests | ⬜ Pending | |
| **M4** | Task 6: Workspace-Wide Verification | ⬜ Pending | |

---

### Task 1: ProjectionStateView Trait & ProjectionConformanceSuite

**Files:**
- Create: `crates/brain-domain/src/projection/conformance.rs`
- Modify: `crates/brain-domain/src/projection/graph_adjacency/reducer.rs`
- Modify: `crates/brain-domain/src/projection/temporal_state/reducer.rs`
- Create: `crates/brain-domain/tests/conformance_tests.rs`
- Modify: `crates/brain-domain/src/projection/mod.rs`

**Interfaces:**
- Consumes: `ProjectionReducer`, `FactEvent`
- Produces: `ProjectionStateView` trait (`type State`, `fn state(&self) -> &Self::State`), `ProjectionConformanceSuite` (`assert_reset_clears_state`, `assert_duplicate_event_idempotency`, `assert_replay_equivalence`)

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/conformance_tests.rs
use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::conformance::*;
use brain_domain::projection::graph_adjacency::*;
use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;
use uuid::Uuid;

#[test]
fn test_graph_adjacency_conformance() {
    let reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
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
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    let event = FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    };

    ProjectionConformanceSuite::assert_reset_clears_state(reducer.clone(), &[event.clone()]);
    ProjectionConformanceSuite::assert_duplicate_event_idempotency(reducer, &event);
}

#[test]
fn test_temporal_state_conformance() {
    let reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
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
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    let event = FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    };

    ProjectionConformanceSuite::assert_reset_clears_state(reducer.clone(), &[event.clone()]);
    ProjectionConformanceSuite::assert_duplicate_event_idempotency(reducer, &event);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test conformance_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::conformance`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/conformance.rs
//! Automated conformance testing trait and harness for ProjectionReducers.

use crate::bkf::events::FactEvent;
use crate::projection::reducer::ProjectionReducer;
use std::fmt::Debug;

/// Trait exposing inspectable domain state for automated projection conformance testing.
pub trait ProjectionStateView: ProjectionReducer {
    /// The underlying state type, which must be default-initializable, cloneable, and comparable.
    type State: Clone + PartialEq + Debug + Default;

    /// Returns a reference to the reducer's current materialized state.
    fn state(&self) -> &Self::State;
}

/// Automated conformance suite testing fundamental ProjectionReducer invariants.
pub struct ProjectionConformanceSuite;

impl ProjectionConformanceSuite {
    /// Asserts that reset() restores the reducer state to bitwise Default state.
    pub fn assert_reset_clears_state<R>(mut reducer: R, sample_events: &[FactEvent])
    where
        R: ProjectionStateView,
    {
        for event in sample_events {
            reducer.apply_event(event).expect("sample event should apply successfully");
        }
        reducer.reset().expect("reset should succeed");
        assert_eq!(
            reducer.state(),
            &R::State::default(),
            "State after reset() must equal Default::default()"
        );
    }

    /// Asserts that applying duplicate events produces identical materialized state (idempotency).
    pub fn assert_duplicate_event_idempotency<R>(mut reducer: R, event: &FactEvent)
    where
        R: ProjectionStateView,
    {
        reducer.apply_event(event).expect("first event should apply successfully");
        let state_after_first = reducer.state().clone();
        reducer.apply_event(event).expect("duplicate event should apply successfully");
        assert_eq!(
            reducer.state(),
            &state_after_first,
            "State after duplicate event must be identical to state after single event"
        );
    }

    /// Asserts that two independent reducers processing identical event streams arrive at identical state.
    pub fn assert_replay_equivalence<R>(mut reducer1: R, mut reducer2: R, events: &[FactEvent])
    where
        R: ProjectionStateView,
    {
        for event in events {
            reducer1.apply_event(event).expect("event should apply successfully on reducer 1");
        }
        for event in events {
            reducer2.apply_event(event).expect("event should apply successfully on reducer 2");
        }
        assert_eq!(
            reducer1.state(),
            reducer2.state(),
            "Replayed reducer states must be 100% identical"
        );
    }
}
```

Implement `ProjectionStateView` for `GraphAdjacencyReducer` in `crates/brain-domain/src/projection/graph_adjacency/reducer.rs`:
```rust
impl ProjectionStateView for GraphAdjacencyReducer {
    type State = GraphAdjacencyState;
    fn state(&self) -> &Self::State {
        &self.state
    }
}
```

Implement `ProjectionStateView` for `TemporalStateReducer` in `crates/brain-domain/src/projection/temporal_state/reducer.rs`:
```rust
impl ProjectionStateView for TemporalStateReducer {
    type State = TemporalState;
    fn state(&self) -> &Self::State {
        &self.state
    }
}
```

Export `conformance` in `crates/brain-domain/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test conformance_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add ProjectionStateView trait and ProjectionConformanceSuite test harness"
```

---

### Task 2: EntityStatistics & EntityStatisticsState

**Files:**
- Create: `crates/brain-domain/src/projection/entity_statistics/models.rs`
- Create: `crates/brain-domain/src/projection/entity_statistics/state.rs`
- Create: `crates/brain-domain/tests/entity_statistics_tests.rs`
- Create: `crates/brain-domain/src/projection/entity_statistics/mod.rs`
- Modify: `crates/brain-domain/src/projection/mod.rs`

**Interfaces:**
- Consumes: `KnowledgeEntityId`, `FactVersionId`, `PredicateId`, `Confidence`, `Timestamp`, `FactLifecycle`, `SemanticAssertion`
- Produces: `EntityStatistics`, `EntityStatisticsState` (`get`, `len`, `is_empty`, `record_fact`, `supersede_fact`, `archive_fact`)

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/entity_statistics_tests.rs
use brain_domain::bkf::*;
use brain_domain::projection::entity_statistics::*;
use uuid::Uuid;

#[test]
fn test_entity_statistics_record_supersede_archive_lifecycle() {
    let mut state = EntityStatisticsState::default();
    let fact_id1 = FactVersionId(Uuid::new_v4());
    let assertion_id1 = AssertionId(Uuid::new_v4());
    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let pred_id1 = PredicateId(Uuid::new_v4());
    let t10 = Timestamp::now();

    let fact1 = FactVersion {
        id: fact_id1.clone(),
        assertion_id: assertion_id1,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(0.8).unwrap(),
        temporal: TemporalWindow::new(t10, t10, t10, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };

    let assertion1 = SemanticAssertion {
        id: assertion_id1,
        kind: AssertionKind::Relationship,
        subject: entity_id.clone(),
        predicate: pred_id1.clone(),
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    state.record_fact(&fact1, &assertion1);

    let stats = state.get(&entity_id).unwrap();
    assert_eq!(stats.total_fact_versions, 1);
    assert_eq!(stats.active_facts_count, 1);
    assert_eq!(stats.unique_predicates_count, 1);
    assert!((stats.average_confidence() - 0.8).abs() < 1e-4);

    let t20 = Timestamp::now();
    state.archive_fact(&fact_id1, t20);

    let stats_after = state.get(&entity_id).unwrap();
    assert_eq!(stats_after.active_facts_count, 0);
    assert_eq!(stats_after.archived_facts_count, 1);
    assert_eq!(stats_after.unique_predicates_count, 0);
    assert_eq!(stats_after.average_confidence(), 0.0);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test entity_statistics_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::entity_statistics`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/entity_statistics/models.rs
//! Data models for Entity Statistics Projection.

use crate::bkf::*;
use serde::{Deserialize, Serialize};

/// Materialized operational summary metrics for a single domain entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityStatistics {
    /// Target entity identifier.
    pub entity_id: KnowledgeEntityId,
    /// Total count of fact versions recorded for this entity.
    pub total_fact_versions: u64,
    /// Count of superseded fact versions.
    pub superseded_facts_count: u64,
    /// Count of archived fact versions.
    pub archived_facts_count: u64,
    /// Count of currently active fact versions.
    pub active_facts_count: usize,
    /// Count of unique predicates associated with currently active facts.
    pub unique_predicates_count: usize,
    /// Timestamp when this entity was first observed in the fact stream.
    pub first_observed_at: Timestamp,
    /// Timestamp when this entity was last updated.
    pub last_updated_at: Timestamp,
    /// Internal sum of active fact confidence scores for exact running mean calculation.
    pub active_confidence_sum: f64,
}

impl EntityStatistics {
    /// Computes the running average confidence across active facts in O(1) time.
    pub fn average_confidence(&self) -> f32 {
        if self.active_facts_count > 0 {
            (self.active_confidence_sum / self.active_facts_count as f64) as f32
        } else {
            0.0
        }
    }
}
```

```rust
// crates/brain-domain/src/projection/entity_statistics/state.rs
//! In-memory entity statistics state.

use crate::bkf::*;
use crate::projection::entity_statistics::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ActiveFactMetadata {
    entity_id: KnowledgeEntityId,
    predicate_id: PredicateId,
    confidence: f64,
}

/// Materialized operational summary state mapping entity IDs to EntityStatistics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityStatisticsState {
    entities: HashMap<KnowledgeEntityId, EntityStatistics>,
    active_facts: HashMap<FactVersionId, ActiveFactMetadata>,
    predicate_refcounts: HashMap<KnowledgeEntityId, HashMap<PredicateId, usize>>,
}

impl EntityStatisticsState {
    /// Returns statistical summary for entity if present.
    pub fn get(&self, entity: &KnowledgeEntityId) -> Option<&EntityStatistics> {
        self.entities.get(entity)
    }

    /// Returns total number of tracked entities.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Returns true if no entities are tracked.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Internal helper processing FactRecorded event. Idempotent on duplicate `fact.id`.
    pub fn record_fact(&mut self, fact: &FactVersion, assertion: &SemanticAssertion) {
        if self.active_facts.contains_key(&fact.id) {
            return;
        }

        let entity_id = assertion.subject.clone();
        let predicate_id = assertion.predicate.clone();
        let confidence_val = fact.confidence.as_f32() as f64;
        let recorded_at = fact.temporal.valid_from;

        self.active_facts.insert(
            fact.id.clone(),
            ActiveFactMetadata {
                entity_id: entity_id.clone(),
                predicate_id: predicate_id.clone(),
                confidence: confidence_val,
            },
        );

        let pred_counts = self.predicate_refcounts.entry(entity_id.clone()).or_default();
        let refcount = pred_counts.entry(predicate_id).or_default();
        let is_new_predicate = *refcount == 0;
        *refcount += 1;

        let stats = self.entities.entry(entity_id.clone()).or_insert_with(|| EntityStatistics {
            entity_id,
            total_fact_versions: 0,
            superseded_facts_count: 0,
            archived_facts_count: 0,
            active_facts_count: 0,
            unique_predicates_count: 0,
            first_observed_at: recorded_at,
            last_updated_at: recorded_at,
            active_confidence_sum: 0.0,
        });

        stats.total_fact_versions += 1;
        stats.active_facts_count += 1;
        stats.active_confidence_sum += confidence_val;
        if is_new_predicate {
            stats.unique_predicates_count += 1;
        }
        stats.last_updated_at = recorded_at;
    }

    /// Internal helper processing FactSuperseded event.
    pub fn supersede_fact(&mut self, old_fact_id: &FactVersionId, superseded_at: Timestamp) {
        if let Some(meta) = self.active_facts.remove(old_fact_id) {
            self.remove_active_metadata(meta, superseded_at, FactLifecycle::Superseded);
        }
    }

    /// Internal helper processing FactArchived event.
    pub fn archive_fact(&mut self, archived_fact_id: &FactVersionId, archived_at: Timestamp) {
        if let Some(meta) = self.active_facts.remove(archived_fact_id) {
            self.remove_active_metadata(meta, archived_at, FactLifecycle::Archived);
        }
    }

    fn remove_active_metadata(&mut self, meta: ActiveFactMetadata, event_time: Timestamp, reason: FactLifecycle) {
        if let Some(stats) = self.entities.get_mut(&meta.entity_id) {
            stats.active_facts_count = stats.active_facts_count.saturating_sub(1);
            stats.active_confidence_sum = (stats.active_confidence_sum - meta.confidence).max(0.0);
            stats.last_updated_at = event_time;

            match reason {
                FactLifecycle::Superseded => stats.superseded_facts_count += 1,
                FactLifecycle::Archived => stats.archived_facts_count += 1,
                _ => {}
            }

            if let Some(pred_counts) = self.predicate_refcounts.get_mut(&meta.entity_id) {
                if let Some(cnt) = pred_counts.get_mut(&meta.predicate_id) {
                    *cnt = cnt.saturating_sub(1);
                    if *cnt == 0 {
                        pred_counts.remove(&meta.predicate_id);
                        stats.unique_predicates_count = stats.unique_predicates_count.saturating_sub(1);
                    }
                }
            }
        }
    }
}
```

Export `models` and `state` in `crates/brain-domain/src/projection/entity_statistics/mod.rs` and export `pub mod entity_statistics;` in `crates/brain-domain/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test entity_statistics_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add EntityStatistics and EntityStatisticsState"
```

---

### Task 3: EntityStatisticsReducer Implementation

**Files:**
- Create: `crates/brain-domain/src/projection/entity_statistics/reducer.rs`
- Modify: `crates/brain-domain/tests/entity_statistics_tests.rs`
- Modify: `crates/brain-domain/tests/conformance_tests.rs`
- Modify: `crates/brain-domain/src/projection/entity_statistics/mod.rs`

**Interfaces:**
- Consumes: `EntityStatisticsState`, `ProjectionReducer`, `ProjectionStateView`, `FactEvent`
- Produces: `EntityStatisticsReducer` (`new`, `id`, `version`, `apply_event`, `reset`, `state`)

- [ ] **Step 1: Write failing test**

```rust
// Add to crates/brain-domain/tests/conformance_tests.rs
#[test]
fn test_entity_statistics_conformance() {
    let reducer = EntityStatisticsReducer::new(ProjectionId::new("statistics"), ProjectionVersion(1));
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
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    let event = FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    };

    ProjectionConformanceSuite::assert_reset_clears_state(reducer.clone(), &[event.clone()]);
    ProjectionConformanceSuite::assert_duplicate_event_idempotency(reducer, &event);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test conformance_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::entity_statistics::EntityStatisticsReducer`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/entity_statistics/reducer.rs
//! Pure domain reducer for Entity Statistics Projection.

use crate::bkf::events::FactEvent;
use crate::projection::conformance::*;
use crate::projection::entity_statistics::state::*;
use crate::projection::errors::*;
use crate::projection::id::*;
use crate::projection::reducer::*;

/// Domain reducer reducing FactEvents into EntityStatisticsState.
#[derive(Debug, Clone)]
pub struct EntityStatisticsReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: EntityStatisticsState,
}

impl EntityStatisticsReducer {
    /// Creates a new EntityStatisticsReducer.
    pub fn new(id: ProjectionId, version: ProjectionVersion) -> Self {
        Self {
            id,
            version,
            state: EntityStatisticsState::default(),
        }
    }
}

impl ProjectionStateView for EntityStatisticsReducer {
    type State = EntityStatisticsState;
    fn state(&self) -> &Self::State {
        &self.state
    }
}

impl ProjectionReducer for EntityStatisticsReducer {
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
            FactEvent::FactSuperseded { old_fact_id, superseded_at, .. } => {
                self.state.supersede_fact(old_fact_id, *superseded_at);
            }
            FactEvent::FactArchived { fact_id, archived_at } => {
                self.state.archive_fact(fact_id, *archived_at);
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.state = EntityStatisticsState::default();
        Ok(())
    }
}
```

Re-export `reducer` in `crates/brain-domain/src/projection/entity_statistics/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test conformance_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): implement EntityStatisticsReducer processing FactEvents"
```

---

### Milestone 2 Checkpoint: Unit & Conformance Tests Freeze

- Verify all domain unit tests pass: `cargo test -p brain-domain`.
- Freeze `brain-domain::projection::entity_statistics` exports.

---

### Task 4: Projection Runtime Service Export (`crates/brain-services`)

**Files:**
- Modify: `crates/brain-services/src/projection/mod.rs`
- Create: `crates/brain-services/tests/entity_statistics_export_tests.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/entity_statistics_export_tests.rs
use brain_domain::projection::entity_statistics::*;
use brain_domain::projection::*;
use brain_services::projection::entity_statistics::*;

#[test]
fn test_entity_statistics_services_reexport() {
    let reducer = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    assert_eq!(reducer.id().as_str(), "stats");
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test entity_statistics_export_tests
```
Expected: FAIL with `unresolved import brain_services::projection::entity_statistics`.

- [ ] **Step 3: Implement minimal code**

Re-export `brain_domain::projection::entity_statistics` in `crates/brain-services/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test entity_statistics_export_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): re-export EntityStatisticsReducer in brain-services::projection"
```

---

### Task 5: Runtime Replay Integration & Catch-Up Tests (`crates/brain-services/tests/entity_statistics_runtime_tests.rs`)

**Files:**
- Create: `crates/brain-services/tests/entity_statistics_runtime_tests.rs`

- [ ] **Step 1: Write runtime replay test**

```rust
// crates/brain-services/tests/entity_statistics_runtime_tests.rs
use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::entity_statistics::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::runtime::*;
use brain_services::projection::store::*;
use uuid::Uuid;

#[test]
fn test_entity_statistics_runtime_replay_equivalence() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(EntityStatisticsReducer::new(
        ProjectionId::new("entity_stats"),
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
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    let events = vec![FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    }];
    runtime.catchup_all(events.iter(), Watermark(1)).unwrap();
}

#[test]
fn test_entity_statistics_mixed_event_sequence() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(EntityStatisticsReducer::new(
        ProjectionId::new("stats_mixed"),
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
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
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
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
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
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test entity_statistics_runtime_tests
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/brain-services/ && git commit -m "test(services): add EntityStatisticsReducer integration and catch-up replay tests"
```

---

### Task 6: Workspace-Wide Verification

- Run `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-domain -p brain-services`.
- Verify clean compilation, 0 test failures, and 0 warnings.
- Update `walkthrough.md`.
