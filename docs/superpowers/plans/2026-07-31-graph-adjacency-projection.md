# Graph Adjacency Projection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 4 — Sub-Project 1: **Graph Adjacency Projection** as a pure domain read model (`GraphAdjacencyState`, `GraphAdjacencyReducer`) maintaining normalized dual-index adjacency lists (`out_edges`, `in_edges`, `edges`, `degrees`) over domain events (`FactEvent`).

**Architecture:** Data models and reducer live in `brain-domain::projection::graph_adjacency` with zero external dependencies. State lookup is encapsulated via domain read methods (`neighbors_out`, `neighbors_in`, `degree`, `edge`). Reducer mutations use atomic helper functions (`insert_edge`, `remove_edge`) with empty key pruning and idempotent duplicate handling. Runtime integration tests live in `brain-services`.

**Tech Stack:** Rust (edition 2021), `serde`, `std::collections::HashMap`.

## Global Constraints
- `brain-domain` must contain zero async runtimes, logger setups, database engines, or network dependencies (`#![deny(missing_docs)]` enabled).
- Adjacency lists must maintain strict deterministic event sequence ordering.
- Deleting the final edge targeting or originating from a node must prune the empty `NodeId` map entry.
- Given identical event streams, live dispatch and catch-up replay must yield 100% bitwise-identical `GraphAdjacencyState`.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: NodeId, EdgeId, EdgeRecord, NodeDegree & GraphAdjacencyState | ✅ Completed | `a25e326` |
| **M1** | Task 2: GraphAdjacencyReducer Implementation | ✅ Completed | `5f8fa90` |
| **M1 Checkpoint** | **Unit & Invariant Tests Freeze** | ✅ Completed | `5f8fa90` |
| **M2** | Task 3: Projection Runtime Service Export | ✅ Completed | `501c959` |
| **M2** | Task 4: Runtime Replay Integration & Catch-Up Tests | ✅ Completed | `3a7c903` |
| **M3** | Task 5: Workspace-Wide Verification | ✅ Completed | `cd20921` |

---

### Task 1: NodeId, EdgeId, EdgeRecord, NodeDegree & GraphAdjacencyState

**Files:**
- Create: `crates/brain-domain/src/projection/graph_adjacency/models.rs`
- Create: `crates/brain-domain/src/projection/graph_adjacency/state.rs`
- Create: `crates/brain-domain/tests/graph_adjacency_state_tests.rs`
- Create: `crates/brain-domain/src/projection/graph_adjacency/mod.rs`
- Modify: `crates/brain-domain/src/projection/mod.rs`

**Interfaces:**
- Consumes: `EntityId`, `FactVersionId`, `PredicateId`, `Confidence`, `TemporalWindow`
- Produces: `NodeId`, `EdgeId`, `EdgeRecord`, `NodeDegree`, `GraphAdjacencyState` (`neighbors_out`, `neighbors_in`, `degree`, `edge`, `insert_edge`, `remove_edge`)

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/graph_adjacency_state_tests.rs
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::*;

#[test]
fn test_graph_adjacency_state_insert_lookup_and_prune() {
    let mut state = GraphAdjacencyState::default();
    let edge_id = EdgeId(FactVersionId::new("f1"));
    let source = NodeId(EntityId::new("e1"));
    let target = NodeId(EntityId::new("e2"));

    let record = EdgeRecord {
        id: edge_id.clone(),
        source: source.clone(),
        target: target.clone(),
        predicate: PredicateId::new("p1"),
        confidence: Confidence::default(),
        temporal: TemporalWindow::default(),
    };

    state.insert_edge(record);
    assert_eq!(state.neighbors_out(&source), &[edge_id.clone()]);
    assert_eq!(state.neighbors_in(&target), &[edge_id.clone()]);
    assert_eq!(state.degree(&source).out_degree, 1);
    assert_eq!(state.degree(&target).in_degree, 1);

    state.remove_edge(&edge_id);
    assert!(state.neighbors_out(&source).is_empty());
    assert!(state.neighbors_in(&target).is_empty());
    assert_eq!(state.degree(&source).out_degree, 0);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test graph_adjacency_state_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::graph_adjacency`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/graph_adjacency/models.rs
//! Data models for Graph Adjacency Projection.

use crate::bkf::*;
use serde::{Deserialize, Serialize};

/// Node identifier wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub EntityId);

/// Edge identifier wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub FactVersionId);

/// Edge record containing normalized edge payload and metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeRecord {
    /// Unique edge ID.
    pub id: EdgeId,
    /// Source node ID.
    pub source: NodeId,
    /// Target node ID.
    pub target: NodeId,
    /// Predicate ID.
    pub predicate: PredicateId,
    /// Confidence score.
    pub confidence: Confidence,
    /// Temporal validity window.
    pub temporal: TemporalWindow,
}

/// Cached degree stats per node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NodeDegree {
    /// Incoming edge count.
    pub in_degree: usize,
    /// Outgoing edge count.
    pub out_degree: usize,
}
```

```rust
// crates/brain-domain/src/projection/graph_adjacency/state.rs
//! In-memory graph adjacency state.

use crate::projection::graph_adjacency::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// In-memory graph adjacency state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphAdjacencyState {
    out_edges: HashMap<NodeId, Vec<EdgeId>>,
    in_edges: HashMap<NodeId, Vec<EdgeId>>,
    edges: HashMap<EdgeId, EdgeRecord>,
    degrees: HashMap<NodeId, NodeDegree>,
}

impl GraphAdjacencyState {
    /// Returns outgoing edge IDs for node.
    pub fn neighbors_out(&self, node: &NodeId) -> &[EdgeId] {
        self.out_edges.get(node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns incoming edge IDs for node.
    pub fn neighbors_in(&self, node: &NodeId) -> &[EdgeId] {
        self.in_edges.get(node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns node degree stats.
    pub fn degree(&self, node: &NodeId) -> NodeDegree {
        self.degrees.get(node).copied().unwrap_or_default()
    }

    /// Returns edge record by EdgeId.
    pub fn edge(&self, id: &EdgeId) -> Option<&EdgeRecord> {
        self.edges.get(id)
    }

    /// Internal helper inserting edge record atomically.
    pub fn insert_edge(&mut self, record: EdgeRecord) {
        if self.edges.contains_key(&record.id) {
            return;
        }
        let edge_id = record.id.clone();
        let source = record.source.clone();
        let target = record.target.clone();

        self.edges.insert(edge_id.clone(), record);
        self.out_edges.entry(source.clone()).or_default().push(edge_id.clone());
        self.in_edges.entry(target.clone()).or_default().push(edge_id);

        self.degrees.entry(source).or_default().out_degree += 1;
        self.degrees.entry(target).or_default().in_degree += 1;
    }

    /// Internal helper removing edge record atomically with empty key pruning.
    pub fn remove_edge(&mut self, edge_id: &EdgeId) {
        if let Some(record) = self.edges.remove(edge_id) {
            if let Some(out_list) = self.out_edges.get_mut(&record.source) {
                out_list.retain(|id| id != edge_id);
                if out_list.is_empty() {
                    self.out_edges.remove(&record.source);
                }
            }
            if let Some(in_list) = self.in_edges.get_mut(&record.target) {
                in_list.retain(|id| id != edge_id);
                if in_list.is_empty() {
                    self.in_edges.remove(&record.target);
                }
            }

            if let Some(deg) = self.degrees.get_mut(&record.source) {
                deg.out_degree = deg.out_degree.saturating_sub(1);
                if deg.out_degree == 0 && deg.in_degree == 0 {
                    self.degrees.remove(&record.source);
                }
            }
            if let Some(deg) = self.degrees.get_mut(&record.target) {
                deg.in_degree = deg.in_degree.saturating_sub(1);
                if deg.out_degree == 0 && deg.in_degree == 0 {
                    self.degrees.remove(&record.target);
                }
            }
        }
    }
}
```

Re-export `models` and `state` in `crates/brain-domain/src/projection/graph_adjacency/mod.rs` and export `pub mod graph_adjacency; pub use graph_adjacency::*;` in `crates/brain-domain/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test graph_adjacency_state_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add NodeId, EdgeId, EdgeRecord, NodeDegree, and GraphAdjacencyState"
```

---

### Task 2: GraphAdjacencyReducer Implementation

**Files:**
- Create: `crates/brain-domain/src/projection/graph_adjacency/reducer.rs`
- Create: `crates/brain-domain/tests/graph_adjacency_reducer_tests.rs`
- Modify: `crates/brain-domain/src/projection/graph_adjacency/mod.rs`

**Interfaces:**
- Consumes: `GraphAdjacencyState`, `ProjectionReducer`, `FactEvent`
- Produces: `GraphAdjacencyReducer` (`new`, `id`, `version`, `apply_event`, `reset`, `state`)

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/graph_adjacency_reducer_tests.rs
use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::*;
use uuid::Uuid;

#[test]
fn test_graph_adjacency_reducer_event_application() {
    let mut reducer = GraphAdjacencyReducer::new(ProjectionId::new("graph_adj"), ProjectionVersion(1));
    let fact_id = FactVersionId(Uuid::new_v4());

    let fact = FactVersion::builder()
        .id(fact_id.clone())
        .entity_id(EntityId::new("e1"))
        .predicate_id(PredicateId::new("knows"))
        .value(FactValue::Entity(EntityId::new("e2")))
        .build();

    let record_event = FactEvent::FactRecorded { fact };
    reducer.apply_event(&record_event).unwrap();

    let node = NodeId(EntityId::new("e1"));
    assert_eq!(reducer.state().neighbors_out(&node).len(), 1);

    let archive_event = FactEvent::FactArchived {
        fact_id,
        archived_at: Timestamp::now(),
    };
    reducer.apply_event(&archive_event).unwrap();
    assert!(reducer.state().neighbors_out(&node).is_empty());
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test graph_adjacency_reducer_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::GraphAdjacencyReducer`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/graph_adjacency/reducer.rs
//! Pure domain reducer for Graph Adjacency Projection.

use crate::bkf::events::FactEvent;
use crate::projection::errors::*;
use crate::projection::graph_adjacency::models::*;
use crate::projection::graph_adjacency::state::*;
use crate::projection::id::*;
use crate::projection::reducer::*;

/// Domain reducer reducing FactEvents into GraphAdjacencyState.
#[derive(Debug, Clone)]
pub struct GraphAdjacencyReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: GraphAdjacencyState,
}

impl GraphAdjacencyReducer {
    /// Creates a new GraphAdjacencyReducer.
    pub fn new(id: ProjectionId, version: ProjectionVersion) -> Self {
        Self {
            id,
            version,
            state: GraphAdjacencyState::default(),
        }
    }

    /// Returns reference to internal graph adjacency state.
    pub fn state(&self) -> &GraphAdjacencyState {
        &self.state
    }
}

impl ProjectionReducer for GraphAdjacencyReducer {
    fn id(&self) -> ProjectionId {
        self.id.clone()
    }

    fn version(&self) -> ProjectionVersion {
        self.version
    }

    fn apply_event(&mut self, event: &FactEvent) -> Result<(), ProjectionError> {
        match event {
            FactEvent::FactRecorded { fact } => {
                let edge_id = EdgeId(fact.id.clone());
                let source = NodeId(fact.entity_id.clone());
                if let Some(target_entity) = fact.value.as_entity_id() {
                    let target = NodeId(target_entity.clone());
                    let record = EdgeRecord {
                        id: edge_id,
                        source,
                        target,
                        predicate: fact.predicate_id.clone(),
                        confidence: fact.confidence,
                        temporal: fact.validity,
                    };
                    self.state.insert_edge(record);
                }
            }
            FactEvent::FactSuperseded { old_fact_id, .. } | FactEvent::FactArchived { fact_id: old_fact_id, .. } => {
                let edge_id = EdgeId(old_fact_id.clone());
                self.state.remove_edge(&edge_id);
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.state = GraphAdjacencyState::default();
        Ok(())
    }
}
```

Re-export `reducer` in `crates/brain-domain/src/projection/graph_adjacency/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test graph_adjacency_reducer_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): implement GraphAdjacencyReducer processing FactEvents"
```

---

### Milestone 1 Checkpoint: Unit & Invariant Tests Freeze

- Verify all domain unit tests pass: `cargo test -p brain-domain`.
- Freeze `brain-domain::projection::graph_adjacency` exports.

---

### Task 3: Projection Runtime Service Export (`crates/brain-services`)

**Files:**
- Modify: `crates/brain-services/src/projection/mod.rs`
- Create: `crates/brain-services/tests/graph_adjacency_export_tests.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/graph_adjacency_export_tests.rs
use brain_domain::projection::*;
use brain_services::projection::graph_adjacency::*;

#[test]
fn test_graph_adjacency_services_reexport() {
    let reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    assert_eq!(reducer.id().as_str(), "adj");
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test graph_adjacency_export_tests
```
Expected: FAIL with `unresolved import brain_services::projection::graph_adjacency`.

- [ ] **Step 3: Implement minimal code**

Re-export `brain_domain::projection::graph_adjacency` in `crates/brain-services/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test graph_adjacency_export_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): re-export GraphAdjacencyReducer in brain-services::projection"
```

---

### Task 4: Runtime Replay Integration & Catch-Up Tests (`crates/brain-services/tests/graph_adjacency_runtime_tests.rs`)

**Files:**
- Create: `crates/brain-services/tests/graph_adjacency_runtime_tests.rs`

- [ ] **Step 1: Write runtime replay test**

```rust
// crates/brain-services/tests/graph_adjacency_runtime_tests.rs
use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::runtime::*;
use brain_services::projection::store::*;
use uuid::Uuid;

#[test]
fn test_graph_adjacency_runtime_replay_equivalence() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(GraphAdjacencyReducer::new(
        ProjectionId::new("graph_adj"),
        ProjectionVersion(1),
    ));
    let instance = ProjectionInstance::new(reducer);
    runtime.register_projection(instance).unwrap();

    let fact_id = FactVersionId(Uuid::new_v4());
    let fact = FactVersion::builder()
        .id(fact_id)
        .entity_id(EntityId::new("e1"))
        .predicate_id(PredicateId::new("rel"))
        .value(FactValue::Entity(EntityId::new("e2")))
        .build();

    let events = vec![FactEvent::FactRecorded { fact }];
    runtime.catchup_all(events.iter(), Watermark(1)).unwrap();
}
```

- [ ] **Step 2: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test graph_adjacency_runtime_tests
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/brain-services/ && git commit -m "test(services): add GraphAdjacencyReducer integration and catch-up replay tests"
```

---

### Task 5: Workspace-Wide Verification

- Run `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-domain -p brain-services`.
- Verify clean compilation, 0 test failures, and 0 warnings.
- Create `walkthrough.md`.
