# Phase 4 — Sub-Project 2: Temporal State Projection Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-domain` (`projection::temporal_state`) & `crates/brain-services` (`projection::temporal_state`)

---

## 1. Executive Summary & Goals

The **Temporal State Projection** is a pure domain read model (`TemporalState`, `TemporalStateReducer`) that materializes validity intervals (`[valid_from, valid_until)`) and entity timelines over `FactEvent` streams. 

Unlike topology projections (e.g. Graph Adjacency), which answer *"what is connected?"*, the Temporal State Projection answers point-in-time and historical queries in $O(1)$ or $O(\log N)$ time:
- **Active facts for an entity** (`active_facts(entity)`)
- **Facts valid at timestamp $T$** (`facts_at(entity, timestamp)`)
- **Full entity timeline** (`timeline(entity)`)
- **Temporal record lookup & status** (`record(id)`, `is_active(id)`)

---

## 2. Architecture & Domain Purity

The projection strictly follows the domain/runtime separation established in previous phases:

```text
FactEvent Stream
       │
       ▼
TemporalStateReducer (brain-domain)
       │
       ▼
TemporalState (Normalized Read Model)
```

- **Zero Subsystem Dependencies**: `brain-domain` contains zero async runtimes, logger setups, database engines, or network modules.
- **Replay & Checkpoint Ignorant**: `TemporalStateReducer` is completely unaware of replay execution, storage checkpoints, or network transport.
- **Single-Writer Safety**: All state updates are deterministic and strictly sequential per event sequence number.
- **Runtime Ordering Assumption**: This projection assumes `FactEvent`s are delivered in canonical event sequence order by the `ProjectionRuntime`.

---

## 3. Data Models (`crates/brain-domain/src/projection/temporal_state/models.rs`)

### `TemporalFactId`
Strongly-typed identifier wrapper around `FactVersionId`.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TemporalFactId(pub FactVersionId);
```

### `TemporalRecord`
Materialized temporal record representing a fact version's validity interval.

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemporalRecord {
    /// Unique fact version ID.
    pub id: TemporalFactId,
    /// Subject entity identifier.
    pub entity_id: KnowledgeEntityId,
    /// Predicate identifier.
    pub predicate_id: PredicateId,
    /// Inclusive beginning of the validity interval.
    pub valid_from: Timestamp,
    /// Exclusive end of the validity interval. `None` indicates an open/active interval ([valid_from, ∞)).
    pub valid_until: Option<Timestamp>,
    /// Explicit fact lifecycle state (e.g. Verified, Archived, Superseded).
    pub lifecycle: FactLifecycle,
    /// Bounded confidence score.
    pub confidence: Confidence,
    /// Predecessor fact version ID in the version lineage chain.
    pub previous_version: Option<FactVersionId>,
}

impl TemporalRecord {
    /// Returns true if the validity interval is open ([valid_from, ∞)).
    pub fn is_active(&self) -> bool {
        self.valid_until.is_none()
    }
}
```

---

## 4. In-Memory State & Normalized Indices (`crates/brain-domain/src/projection/temporal_state/state.rs`)

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemporalState {
    /// Single source of truth for canonical temporal records.
    records: HashMap<TemporalFactId, TemporalRecord>,
    /// Primary timeline index mapping each entity to its chronological list of fact IDs (append-only).
    entity_timelines: HashMap<KnowledgeEntityId, Vec<TemporalFactId>>,
    /// Fast current-state index mapping each entity to its currently active fact IDs.
    active: HashMap<KnowledgeEntityId, Vec<TemporalFactId>>,
}
```

### Encapsulated Read APIs

```rust
impl TemporalState {
    /// Returns all currently active fact IDs for an entity.
    pub fn active_facts(&self, entity: &KnowledgeEntityId) -> &[TemporalFactId] {
        self.active.get(entity).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns the full chronological timeline of fact IDs for an entity.
    pub fn timeline(&self, entity: &KnowledgeEntityId) -> &[TemporalFactId] {
        self.entity_timelines.get(entity).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns a reference to a specific temporal record by FactId.
    pub fn record(&self, id: &TemporalFactId) -> Option<&TemporalRecord> {
        self.records.get(id)
    }

    /// Returns true if the fact ID is currently active.
    pub fn is_active(&self, id: &TemporalFactId) -> bool {
        self.record(id).map_or(false, TemporalRecord::is_active)
    }

    /// Returns references to facts for an entity that were valid at `at_timestamp`.
    /// A record is valid at T if valid_from <= T and (valid_until > T or valid_until is None).
    pub fn facts_at(&self, entity: &KnowledgeEntityId, at_timestamp: Timestamp) -> Vec<&TemporalRecord> {
        let mut result = Vec::new();
        if let Some(fact_ids) = self.entity_timelines.get(entity) {
            for fact_id in fact_ids {
                if let Some(rec) = self.records.get(fact_id) {
                    if rec.valid_from <= at_timestamp {
                        match rec.valid_until {
                            None => result.push(rec),
                            Some(until) if until > at_timestamp => result.push(rec),
                            _ => {}
                        }
                    }
                }
            }
        }
        result
    }
}
```

### Mutation Helpers with Invariants

```rust
impl TemporalState {
    /// Inserts a new temporal record atomically. Idempotent on duplicate `record.id`.
    pub fn insert_record(&mut self, record: TemporalRecord) {
        if self.records.contains_key(&record.id) {
            return; // Idempotent ignore during catch-up replay
        }
        let fact_id = record.id.clone();
        let entity = record.entity_id.clone();
        let is_active = record.is_active();

        self.records.insert(fact_id.clone(), record);
        self.entity_timelines.entry(entity.clone()).or_default().push(fact_id.clone());
        if is_active {
            self.active.entry(entity).or_default().push(fact_id);
        }
    }

    /// Closes an open validity interval at `closed_at` timestamp with updated lifecycle.
    /// Idempotent if interval is already closed.
    pub fn close_interval(&mut self, id: &TemporalFactId, closed_at: Timestamp, new_lifecycle: FactLifecycle) {
        if let Some(record) = self.records.get_mut(id) {
            if record.valid_until.is_some() {
                return; // Already closed
            }
            debug_assert!(closed_at >= record.valid_from, "closed_at must be monotonic with valid_from");
            record.valid_until = Some(closed_at);
            record.lifecycle = new_lifecycle;

            let entity = record.entity_id.clone();
            if let Some(active_list) = self.active.get_mut(&entity) {
                active_list.retain(|active_id| active_id != id);
                if active_list.is_empty() {
                    self.active.remove(&entity);
                }
            }
        }
    }
}
```

---

## 5. Reducer Contract & Event Handling (`crates/brain-domain/src/projection/temporal_state/reducer.rs`)

```rust
#[derive(Debug, Clone)]
pub struct TemporalStateReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: TemporalState,
}

impl TemporalStateReducer {
    pub fn new(id: ProjectionId, version: ProjectionVersion) -> Self {
        Self {
            id,
            version,
            state: TemporalState::default(),
        }
    }

    pub fn state(&self) -> &TemporalState {
        &self.state
    }
}

impl ProjectionReducer for TemporalStateReducer {
    fn id(&self) -> ProjectionId { self.id.clone() }
    fn version(&self) -> ProjectionVersion { self.version }

    fn apply_event(&mut self, event: &FactEvent) -> Result<(), ProjectionError> {
        match event {
            FactEvent::FactRecorded { fact, assertion } => {
                // Intentionally skip facts that do not carry semantic subject assertion context
                if let Some(assert) = assertion {
                    let fact_id = TemporalFactId(fact.id.clone());
                    let record = TemporalRecord {
                        id: fact_id,
                        entity_id: assert.subject,
                        predicate_id: assert.predicate,
                        valid_from: fact.temporal.valid_from,
                        valid_until: fact.temporal.valid_to,
                        lifecycle: fact.lifecycle,
                        confidence: fact.confidence,
                        previous_version: fact.supersedes,
                    };
                    self.state.insert_record(record);
                }
            }
            FactEvent::FactSuperseded { old_fact_id, superseded_at, .. } => {
                let old_id = TemporalFactId(old_fact_id.clone());
                self.state.close_interval(&old_id, *superseded_at, FactLifecycle::Superseded);
            }
            FactEvent::FactArchived { fact_id, archived_at } => {
                let archived_id = TemporalFactId(fact_id.clone());
                self.state.close_interval(&archived_id, *archived_at, FactLifecycle::Archived);
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.state = TemporalState::default();
        Ok(())
    }
}
```

---

## 6. System & Invariant Rules

1. **Active Interval Consistency**: Every fact in `active` map has `valid_until == None` and `is_active() == true`.
2. **Interval Monotonicity**: `closed_at >= valid_from` for all closed intervals.
3. **Append-Only Entity Timelines**: Entity timelines are append-only; existing entries are never reordered or removed.
4. **Empty Key Pruning**: When an entity's active fact vector becomes empty, the `KnowledgeEntityId` key is removed from `active`.
5. **Deterministic Timeline Order**: Entity timelines store `TemporalFactId`s strictly in canonical event sequence order.
6. **Replay Equivalence**: Replaying identical `FactEvent` streams yields 100% bitwise-identical `TemporalState`.

---

## 7. Out of Scope

The following capabilities are explicitly deferred to maintain a focused Phase 4.2 scope:
- Global predicate timeline indices (`PredicateId` timelines across all entities).
- Bitemporal indices (`valid_time` vs `system_time`).
- Specialized temporal analytics or interval tree search indices.

---

## 8. Verification & Testing Plan

### Unit & Invariant Tests (`crates/brain-domain/tests/temporal_state_tests.rs`)
- `test_temporal_state_record_insert_and_active_lookup`
- `test_temporal_state_close_interval_and_pruning`
- `test_temporal_state_facts_at_point_in_time_queries`
- `test_temporal_state_invariants` (Active index integrity, monotonic bounds, timeline ordering)

### Service Runtime Integration Tests (`crates/brain-services/tests/temporal_state_runtime_tests.rs`)
- `test_temporal_state_runtime_replay_equivalence`
- `test_temporal_state_mixed_event_sequence` (`FactRecorded` $\rightarrow$ `FactSuperseded` $\rightarrow$ `FactArchived`)
