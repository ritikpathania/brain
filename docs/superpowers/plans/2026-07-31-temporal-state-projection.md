# Temporal State Projection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 4 — Sub-Project 2: **Temporal State Projection** as a pure domain read model (`TemporalState`, `TemporalStateReducer`) materializing validity intervals (`[valid_from, valid_until)`) and entity timelines over `FactEvent` streams.

**Architecture:** Data models, normalized state maps, and reducer live in `brain-domain::projection::temporal_state` with zero external dependencies. State lookup is encapsulated via domain read methods (`active_facts`, `timeline`, `record`, `is_active`, `facts_at`). Reducer mutations use atomic helper functions (`insert_record`, `close_interval`) enforcing interval monotonicity and empty key pruning. Runtime integration tests live in `brain-services`.

**Tech Stack:** Rust (edition 2021), `serde`, `std::collections::HashMap`.

## Global Constraints
- `brain-domain` must contain zero async runtimes, logger setups, database engines, or network dependencies (`#![deny(missing_docs)]` enabled).
- Entity timelines must maintain strict deterministic event sequence ordering (append-only).
- Closing a record's validity interval must remove its ID from `active` and prune the `KnowledgeEntityId` map key if active list becomes empty.
- Given identical event streams, live dispatch and catch-up replay must yield 100% bitwise-identical `TemporalState`.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: TemporalFactId, TemporalRecord, and TemporalState | ⬜ Pending | |
| **M1** | Task 2: TemporalStateReducer Implementation | ⬜ Pending | |
| **M1 Checkpoint** | **Unit & Invariant Tests Freeze** | ⬜ Pending | |
| **M2** | Task 3: Projection Runtime Service Export | ⬜ Pending | |
| **M2** | Task 4: Runtime Replay Integration & Catch-Up Tests | ⬜ Pending | |
| **M3** | Task 5: Workspace-Wide Verification | ⬜ Pending | |

---

### Task 1: TemporalFactId, TemporalRecord, and TemporalState

**Files:**
- Create: `crates/brain-domain/src/projection/temporal_state/models.rs`
- Create: `crates/brain-domain/src/projection/temporal_state/state.rs`
- Create: `crates/brain-domain/tests/temporal_state_tests.rs`
- Create: `crates/brain-domain/src/projection/temporal_state/mod.rs`
- Modify: `crates/brain-domain/src/projection/mod.rs`

**Interfaces:**
- Consumes: `KnowledgeEntityId`, `FactVersionId`, `PredicateId`, `Confidence`, `Timestamp`, `FactLifecycle`
- Produces: `TemporalFactId`, `TemporalRecord`, `TemporalState` (`active_facts`, `timeline`, `record`, `is_active`, `facts_at`, `insert_record`, `close_interval`)

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/temporal_state_tests.rs
use brain_domain::bkf::*;
use brain_domain::projection::temporal_state::*;
use uuid::Uuid;

#[test]
fn test_temporal_state_record_insert_close_and_point_in_time_lookup() {
    let mut state = TemporalState::default();
    let fact_id = TemporalFactId(FactVersionId(Uuid::new_v4()));
    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let predicate_id = PredicateId(Uuid::new_v4());
    let t10 = Timestamp::from_unix_seconds(10);
    let t20 = Timestamp::from_unix_seconds(20);

    let record = TemporalRecord {
        id: fact_id.clone(),
        entity_id: entity_id.clone(),
        predicate_id: predicate_id.clone(),
        valid_from: t10,
        valid_until: None,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        previous_version: None,
    };

    // Test insert and active status
    state.insert_record(record.clone());
    assert_eq!(state.active_facts(&entity_id), &[fact_id.clone()]);
    assert_eq!(state.timeline(&entity_id), &[fact_id.clone()]);
    assert!(state.is_active(&fact_id));

    // Test duplicate insertion idempotency
    state.insert_record(record);
    assert_eq!(state.timeline(&entity_id).len(), 1);

    // Test close interval & duplicate closure idempotency
    state.close_interval(&fact_id, t20, FactLifecycle::Archived);
    state.close_interval(&fact_id, t20, FactLifecycle::Archived);
    assert!(state.active_facts(&entity_id).is_empty());
    assert!(!state.is_active(&fact_id));
    assert_eq!(state.record(&fact_id).unwrap().valid_until, Some(t20));

    // Test half-open interval boundaries [10, 20)
    assert!(state.facts_at(&entity_id, Timestamp::from_unix_seconds(9)).is_empty());
    assert_eq!(state.facts_at(&entity_id, Timestamp::from_unix_seconds(10)).len(), 1);
    assert_eq!(state.facts_at(&entity_id, Timestamp::from_unix_seconds(19)).len(), 1);
    assert!(state.facts_at(&entity_id, Timestamp::from_unix_seconds(20)).is_empty());
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test temporal_state_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::temporal_state`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/temporal_state/models.rs
//! Data models for Temporal State Projection.

use crate::bkf::*;
use serde::{Deserialize, Serialize};

/// Wrapper around FactVersionId for temporal projection indexing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TemporalFactId(pub FactVersionId);

/// Materialized temporal record representing a fact version's validity interval.
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
    /// Exclusive end of the validity interval. None indicates an open/active interval ([valid_from, ∞)).
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

```rust
// crates/brain-domain/src/projection/temporal_state/state.rs
//! In-memory temporal state.

use crate::bkf::*;
use crate::projection::temporal_state::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Materialized temporal state over entity timelines and validity intervals.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemporalState {
    records: HashMap<TemporalFactId, TemporalRecord>,
    entity_timelines: HashMap<KnowledgeEntityId, Vec<TemporalFactId>>,
    active: HashMap<KnowledgeEntityId, Vec<TemporalFactId>>,
}

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

    /// Inserts a new temporal record atomically. Idempotent on duplicate `record.id`.
    pub fn insert_record(&mut self, record: TemporalRecord) {
        if self.records.contains_key(&record.id) {
            return;
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
    pub fn close_interval(&mut self, id: &TemporalFactId, closed_at: Timestamp, new_lifecycle: FactLifecycle) {
        if let Some(record) = self.records.get_mut(id) {
            if record.valid_until.is_some() {
                return;
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

Export `models` and `state` in `crates/brain-domain/src/projection/temporal_state/mod.rs` and export `pub mod temporal_state;` in `crates/brain-domain/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test temporal_state_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add TemporalFactId, TemporalRecord, and TemporalState"
```

---

### Task 2: TemporalStateReducer Implementation

**Files:**
- Create: `crates/brain-domain/src/projection/temporal_state/reducer.rs`
- Modify: `crates/brain-domain/tests/temporal_state_tests.rs`
- Modify: `crates/brain-domain/src/projection/temporal_state/mod.rs`

**Interfaces:**
- Consumes: `TemporalState`, `ProjectionReducer`, `FactEvent`
- Produces: `TemporalStateReducer` (`new`, `id`, `version`, `apply_event`, `reset`, `state`)

- [ ] **Step 1: Write failing test**

```rust
// Add to crates/brain-domain/tests/temporal_state_tests.rs
#[test]
fn test_temporal_state_reducer_event_application_and_reset() {
    let mut reducer = TemporalStateReducer::new(ProjectionId::new("temporal_state"), ProjectionVersion(1));
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

    let record_event1 = FactEvent::FactRecorded {
        fact: fact1,
        assertion: Some(assertion1),
    };
    reducer.apply_event(&record_event1).unwrap();

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

    let record_event2 = FactEvent::FactRecorded {
        fact: fact2,
        assertion: Some(assertion2),
    };
    reducer.apply_event(&record_event2).unwrap();

    // Verify previous_version lineage preservation
    let rec2 = reducer.state().record(&TemporalFactId(fact_id2)).unwrap();
    assert_eq!(rec2.previous_version, Some(fact_id1.clone()));

    // Test reset() empties state
    reducer.reset().unwrap();
    assert!(reducer.state().timeline(&entity_id).is_empty());
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test temporal_state_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::temporal_state::TemporalStateReducer`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/temporal_state/reducer.rs
//! Pure domain reducer for Temporal State Projection.

use crate::bkf::events::FactEvent;
use crate::bkf::fact_version::*;
use crate::projection::errors::*;
use crate::projection::id::*;
use crate::projection::reducer::*;
use crate::projection::temporal_state::models::*;
use crate::projection::temporal_state::state::*;

/// Domain reducer reducing FactEvents into TemporalState.
#[derive(Debug, Clone)]
pub struct TemporalStateReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: TemporalState,
}

impl TemporalStateReducer {
    /// Creates a new TemporalStateReducer.
    pub fn new(id: ProjectionId, version: ProjectionVersion) -> Self {
        Self {
            id,
            version,
            state: TemporalState::default(),
        }
    }

    /// Returns reference to internal temporal state.
    pub fn state(&self) -> &TemporalState {
        &self.state
    }
}

impl ProjectionReducer for TemporalStateReducer {
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

Re-export `reducer` in `crates/brain-domain/src/projection/temporal_state/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test temporal_state_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): implement TemporalStateReducer processing FactEvents"
```

---

### Milestone 1 Checkpoint: Unit & Invariant Tests Freeze

- Verify all domain unit tests pass: `cargo test -p brain-domain`.
- Freeze `brain-domain::projection::temporal_state` exports.

---

### Task 3: Projection Runtime Service Export (`crates/brain-services`)

**Files:**
- Modify: `crates/brain-services/src/projection/mod.rs`
- Create: `crates/brain-services/tests/temporal_state_export_tests.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/temporal_state_export_tests.rs
use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;
use brain_services::projection::temporal_state::*;

#[test]
fn test_temporal_state_services_reexport() {
    let reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    assert_eq!(reducer.id().as_str(), "temporal");
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test temporal_state_export_tests
```
Expected: FAIL with `unresolved import brain_services::projection::temporal_state`.

- [ ] **Step 3: Implement minimal code**

Re-export `brain_domain::projection::temporal_state` in `crates/brain-services/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test temporal_state_export_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): re-export TemporalStateReducer in brain-services::projection"
```

---

### Task 4: Runtime Replay Integration & Catch-Up Tests (`crates/brain-services/tests/temporal_state_runtime_tests.rs`)

**Files:**
- Create: `crates/brain-services/tests/temporal_state_runtime_tests.rs`

- [ ] **Step 1: Write runtime replay test**

```rust
// crates/brain-services/tests/temporal_state_runtime_tests.rs
use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::runtime::*;
use brain_services::projection::store::*;
use uuid::Uuid;

#[test]
fn test_temporal_state_runtime_replay_equivalence() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(TemporalStateReducer::new(
        ProjectionId::new("temporal_state"),
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
fn test_temporal_state_mixed_event_sequence() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(TemporalStateReducer::new(
        ProjectionId::new("temporal_mixed"),
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
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test temporal_state_runtime_tests
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/brain-services/ && git commit -m "test(services): add TemporalStateReducer integration and catch-up replay tests"
```

---

### Task 5: Workspace-Wide Verification

- Run `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-domain -p brain-services`.
- Verify clean compilation, 0 test failures, and 0 warnings.
- Update `walkthrough.md`.
