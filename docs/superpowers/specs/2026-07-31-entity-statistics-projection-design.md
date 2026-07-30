# Phase 4 — Sub-Project 3: Entity Statistics Projection & Conformance Suite Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-domain` (`projection::conformance`, `projection::entity_statistics`) & `crates/brain-services` (`projection::entity_statistics`)

---

## 1. Executive Summary & Goals

The **Entity Statistics Projection** is a pure domain read model (`EntityStatisticsState`, `EntityStatisticsReducer`) that materializes operational statistical summaries per entity over `FactEvent` streams in $O(1)$ time:
- **Lifetime metrics**: `total_fact_versions`, `superseded_facts_count`, `archived_facts_count`
- **Current state metrics**: `active_facts_count`, `unique_predicates_count`
- **Temporal metadata**: `first_observed_at`, `last_updated_at`
- **Active knowledge quality**: `average_confidence` (dynamic mean across active facts)

Additionally, this sub-project introduces the reusable **`ProjectionConformanceSuite`** test harness and `ProjectionStateView` trait, enabling automated verification of state reset, duplicate event idempotency, and replay equivalence across all present and future projections.

---

## 2. Architecture & Domain Purity

```text
FactEvent Stream
       │
       ▼
EntityStatisticsReducer (brain-domain)
       │
       ▼
EntityStatisticsState (Normalized Read Model)
```

- **Zero Subsystem Dependencies**: `brain-domain` contains zero async runtimes, logger setups, database engines, or network modules.
- **Replay & Checkpoint Ignorant**: `EntityStatisticsReducer` is completely unaware of replay execution or storage checkpoints.
- **Single-Writer Safety**: All state updates are deterministic and strictly sequential per event sequence number.
- **Floating-Point Equality Determinism**: Exact `PartialEq` state equality in `ProjectionConformanceSuite` assumes canonical event sequence order.

---

## 3. Projection Conformance Suite (`crates/brain-domain/src/projection/conformance.rs`)

```rust
/// Trait exposing inspectable domain state for automated projection conformance testing.
pub trait ProjectionStateView: ProjectionReducer {
    /// The underlying state type, which must be default-initializable, cloneable, and comparable.
    type State: Clone + PartialEq + std::fmt::Debug + Default;

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
        assert_eq!(reducer.state(), &R::State::default(), "State after reset() must equal Default::default()");
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

---

## 4. Entity Statistics Models (`crates/brain-domain/src/projection/entity_statistics/models.rs`)

```rust
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

---

## 5. In-Memory State & Normalized Indices (`crates/brain-domain/src/projection/entity_statistics/state.rs`)

> **Note on Private Implementation Details**: `ActiveFactMetadata`, `active_facts`, and `predicate_refcounts` are derived internal indices whose sole purpose is updating public statistics efficiently. They are not exposed in the external public API.

```rust
/// Internal metadata tracked per active fact version for state updates upon supersession/archival.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ActiveFactMetadata {
    entity_id: KnowledgeEntityId,
    predicate_id: PredicateId,
    confidence: f64,
}

/// Materialized operational summary state mapping entity IDs to EntityStatistics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityStatisticsState {
    /// Primary map from entity ID to its materialized statistics.
    entities: HashMap<KnowledgeEntityId, EntityStatistics>,
    /// Internal lookup map from FactVersionId to active fact metadata.
    active_facts: HashMap<FactVersionId, ActiveFactMetadata>,
    /// Internal reference counts per entity for predicate uniqueness calculation.
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
            return; // Idempotent ignore during replay
        }

        let entity_id = assertion.subject.clone();
        let predicate_id = assertion.predicate.clone();
        let confidence_val = fact.confidence.as_f32() as f64;
        let recorded_at = fact.temporal.valid_from;

        // Register active fact metadata
        self.active_facts.insert(
            fact.id.clone(),
            ActiveFactMetadata {
                entity_id: entity_id.clone(),
                predicate_id: predicate_id.clone(),
                confidence: confidence_val,
            },
        );

        // Update predicate refcount and unique count
        let pred_counts = self.predicate_refcounts.entry(entity_id.clone()).or_default();
        let refcount = pred_counts.entry(predicate_id).or_default();
        let is_new_predicate = *refcount == 0;
        *refcount += 1;

        // Update entity statistics
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

            // Decrement predicate refcount and update unique count
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

---

## 6. Reducer Contract (`crates/brain-domain/src/projection/entity_statistics/reducer.rs`)

```rust
#[derive(Debug, Clone)]
pub struct EntityStatisticsReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: EntityStatisticsState,
}

impl EntityStatisticsReducer {
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
    fn id(&self) -> ProjectionId { self.id.clone() }
    fn version(&self) -> ProjectionVersion { self.version }

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

---

## 7. Verification & Testing Plan

### 1. Conformance Suite Tests (`crates/brain-domain/tests/conformance_tests.rs`)
- `test_graph_adjacency_conformance`: Runs `ProjectionConformanceSuite` for `GraphAdjacencyReducer`.
- `test_temporal_state_conformance`: Runs `ProjectionConformanceSuite` for `TemporalStateReducer`.
- `test_entity_statistics_conformance`: Runs `ProjectionConformanceSuite` for `EntityStatisticsReducer`.

### 2. Entity Statistics Unit & Invariant Tests (`crates/brain-domain/tests/entity_statistics_tests.rs`)
- `test_entity_statistics_record_supersede_archive_lifecycle`
- `test_entity_statistics_unique_predicates_and_average_confidence`
- `test_entity_statistics_idempotency_duplicate_events` (testing duplicate `FactRecorded`, `FactSuperseded`, and `FactArchived` after metadata removal)
- `test_entity_statistics_invariants`

### 3. Service Runtime Integration Tests (`crates/brain-services/tests/entity_statistics_runtime_tests.rs`)
- `test_entity_statistics_runtime_replay_equivalence`
- `test_entity_statistics_mixed_event_sequence`
