# Phase 5.2 — KnowledgeQueryFacade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 5.2 **`KnowledgeQueryFacade`** & **`ProjectionSnapshot`** in `crates/brain-services/src/query/`, establishing an atomic, lock-free, zero-copy read query facade over Phase 4 read model snapshots and stateless query evaluators.

**Architecture:** `ProjectionSnapshot` encapsulates shared `Arc` handles to all four read models (`graph_adjacency`, `temporal_state`, `entity_statistics`, `search_index`) and snapshot `Watermark`. `KnowledgeQueryFacade` uses `ArcSwap<ProjectionSnapshot>` for lock-free reader loads and atomic snapshot publication. Facade delegates to stateless evaluators in `src/query/evaluators/` (`neighborhood.rs`, `temporal.rs`, `search.rs`, `hybrid.rs`).

**Tech Stack:** Rust (edition 2021), `arc-swap = "1.7"`, `parking_lot`, `thiserror`, `uuid`.

## Global Constraints
- `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services` must pass cleanly with zero errors.
- Published `ProjectionSnapshot` instances MUST BE 100% immutable.
- Readers must execute lock-free via `ArcSwap::load_full()`.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: `arc-swap` Dependency & `ProjectionSnapshot` Container | ⬜ Pending | |
| **M2** | Task 2: Stateless Evaluators & `KnowledgeQueryFacade` Integration | ⬜ Pending | |
| **M2 Checkpoint** | **Integration & Concurrency Contract Verification** | ⬜ Pending | |

---

### Task 1: `arc-swap` Dependency & `ProjectionSnapshot` Container

**Files:**
- Modify: `crates/brain-services/Cargo.toml`
- Create: `crates/brain-services/src/query/snapshot.rs`
- Modify: `crates/brain-services/src/query/mod.rs`
- Create: `crates/brain-services/tests/projection_snapshot_tests.rs`

- [ ] **Step 1: Add `arc-swap = "1.7"` to Cargo.toml**

```toml
arc-swap = "1.7"
```

- [ ] **Step 2: Write failing test `crates/brain-services/tests/projection_snapshot_tests.rs`**

```rust
use brain_domain::projection::Watermark;
use brain_services::query::snapshot::ProjectionSnapshot;

#[test]
fn test_projection_snapshot_accessors_and_watermark() {
    let snapshot = ProjectionSnapshot::empty(Watermark(42));
    assert_eq!(snapshot.watermark(), Watermark(42));
    assert!(snapshot.graph().is_empty());
    assert!(snapshot.temporal().is_empty());
    assert!(snapshot.statistics().is_empty());
    assert!(snapshot.search().is_empty());
}
```

- [ ] **Step 3: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_snapshot_tests
```
Expected: FAIL (unresolved import `ProjectionSnapshot`).

- [ ] **Step 4: Implement `ProjectionSnapshot` in `src/query/snapshot.rs` and export in `src/query/mod.rs`**

```rust
// crates/brain-services/src/query/snapshot.rs
use brain_domain::projection::entity_statistics::EntityStatisticsState;
use brain_domain::projection::graph_adjacency::GraphAdjacencyState;
use brain_domain::projection::search_index::SearchIndexState;
use brain_domain::projection::temporal_state::TemporalState;
use brain_domain::projection::Watermark;
use std::sync::Arc;

/// Atomic, immutable snapshot of all four domain read models and stream watermark.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionSnapshot {
    graph_adjacency: Arc<GraphAdjacencyState>,
    temporal_state: Arc<TemporalState>,
    entity_statistics: Arc<EntityStatisticsState>,
    search_index: Arc<SearchIndexState>,
    watermark: Watermark,
}

impl ProjectionSnapshot {
    /// Constructs a new immutable ProjectionSnapshot wrapping shared read model references and watermark.
    pub fn new(
        graph_adjacency: Arc<GraphAdjacencyState>,
        temporal_state: Arc<TemporalState>,
        entity_statistics: Arc<EntityStatisticsState>,
        search_index: Arc<SearchIndexState>,
        watermark: Watermark,
    ) -> Self {
        Self {
            graph_adjacency,
            temporal_state,
            entity_statistics,
            search_index,
            watermark,
        }
    }

    /// Constructs an empty bootstrap ProjectionSnapshot.
    pub fn empty(watermark: Watermark) -> Self {
        Self {
            graph_adjacency: Arc::new(GraphAdjacencyState::default()),
            temporal_state: Arc::new(TemporalState::default()),
            entity_statistics: Arc::new(EntityStatisticsState::default()),
            search_index: Arc::new(SearchIndexState::default()),
            watermark,
        }
    }

    /// Accessor for Graph Adjacency read model.
    pub fn graph(&self) -> &Arc<GraphAdjacencyState> {
        &self.graph_adjacency
    }

    /// Accessor for Temporal State read model.
    pub fn temporal(&self) -> &Arc<TemporalState> {
        &self.temporal_state
    }

    /// Accessor for Entity Statistics read model.
    pub fn statistics(&self) -> &Arc<EntityStatisticsState> {
        &self.entity_statistics
    }

    /// Accessor for Search Index read model.
    pub fn search(&self) -> &Arc<SearchIndexState> {
        &self.search_index
    }

    /// Accessor for Snapshot Watermark.
    pub fn watermark(&self) -> Watermark {
        self.watermark
    }
}
```

Update `crates/brain-services/src/query/mod.rs` to add `pub mod snapshot; pub use snapshot::*;`.

- [ ] **Step 5: Run test to verify PASS**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_snapshot_tests
```
Expected: PASS cleanly.

- [ ] **Step 6: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): add ProjectionSnapshot container in brain-services::query"
```

---

### Task 2: Stateless Evaluators & `KnowledgeQueryFacade` Integration

**Files:**
- Create: `crates/brain-services/src/query/evaluators/mod.rs`
- Create: `crates/brain-services/src/query/evaluators/neighborhood.rs`
- Create: `crates/brain-services/src/query/evaluators/temporal.rs`
- Create: `crates/brain-services/src/query/evaluators/search.rs`
- Create: `crates/brain-services/src/query/evaluators/hybrid.rs`
- Create: `crates/brain-services/src/query/facade.rs`
- Modify: `crates/brain-services/src/query/mod.rs`
- Create: `crates/brain-services/tests/knowledge_query_facade_tests.rs`

- [ ] **Step 1: Write failing test in `crates/brain-services/tests/knowledge_query_facade_tests.rs`**

```rust
use brain_domain::bkf::*;
use brain_domain::projection::Watermark;
use brain_services::query::*;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn test_knowledge_query_facade_lifecycle_and_evaluators() {
    let snapshot_v1 = Arc::new(ProjectionSnapshot::empty(Watermark(10)));
    let facade = KnowledgeQueryFacade::new(snapshot_v1);

    assert_eq!(facade.active_snapshot().watermark(), Watermark(10));

    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let query = NeighborhoodQuery {
        root_entity: entity_id,
        max_hops: 1,
        temporal_mode: TemporalMode::CurrentActive,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res = facade.query_neighborhood(&query).unwrap();
    assert_eq!(res.metadata.snapshot_watermark, 10);

    // Atomic snapshot update
    let snapshot_v2 = Arc::new(ProjectionSnapshot::empty(Watermark(20)));
    facade.update_snapshot(snapshot_v2);

    assert_eq!(facade.active_snapshot().watermark(), Watermark(20));
    let res2 = facade.query_neighborhood(&query).unwrap();
    assert_eq!(res2.metadata.snapshot_watermark, 20);
}

#[test]
fn test_knowledge_query_facade_concurrency_safety() {
    let snapshot_v1 = Arc::new(ProjectionSnapshot::empty(Watermark(100)));
    let facade = Arc::new(KnowledgeQueryFacade::new(snapshot_v1));

    let reader_handle = facade.active_snapshot();
    
    // Writer publishes new snapshot
    let snapshot_v2 = Arc::new(ProjectionSnapshot::empty(Watermark(200)));
    facade.update_snapshot(snapshot_v2);

    // Reader holding snapshot_v1 remains completely unaffected
    assert_eq!(reader_handle.watermark(), Watermark(100));
    assert_eq!(facade.active_snapshot().watermark(), Watermark(200));
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test knowledge_query_facade_tests
```
Expected: FAIL (unresolved import `KnowledgeQueryFacade`).

- [ ] **Step 3: Implement evaluators in `src/query/evaluators/`**

Create `src/query/evaluators/neighborhood.rs`:
```rust
use crate::query::errors::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;

/// Stateless evaluator for node neighborhood graph traversal.
pub struct NeighborhoodEvaluator;

impl NeighborhoodEvaluator {
    /// Evaluates neighborhood query against snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        _query: &NeighborhoodQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        Ok(QueryFacadeResult {
            matches: vec![],
            total_matched: 0,
            metadata: QueryResponseMetadata {
                execution_duration_us: 0,
                snapshot_watermark: snapshot.watermark().0,
            },
        })
    }
}
```

Create `src/query/evaluators/temporal.rs`:
```rust
use crate::query::errors::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;

/// Stateless evaluator for point-in-time entity state lookups.
pub struct TemporalEvaluator;

impl TemporalEvaluator {
    /// Evaluates point-in-time query against snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        _query: &PointInTimeQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        Ok(QueryFacadeResult {
            matches: vec![],
            total_matched: 0,
            metadata: QueryResponseMetadata {
                execution_duration_us: 0,
                snapshot_watermark: snapshot.watermark().0,
            },
        })
    }
}
```

Create `src/query/evaluators/search.rs`:
```rust
use crate::query::errors::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;

/// Stateless evaluator for lexical search queries.
pub struct SearchEvaluator;

impl SearchEvaluator {
    /// Evaluates search query against snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        _query: &LexicalSearchQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        Ok(QueryFacadeResult {
            matches: vec![],
            total_matched: 0,
            metadata: QueryResponseMetadata {
                execution_duration_us: 0,
                snapshot_watermark: snapshot.watermark().0,
            },
        })
    }
}
```

Create `src/query/evaluators/hybrid.rs`:
```rust
use crate::query::errors::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;

/// Stateless evaluator for compound hybrid queries.
pub struct HybridEvaluator;

impl HybridEvaluator {
    /// Evaluates hybrid query against snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        _query: &HybridSearchQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        Ok(QueryFacadeResult {
            matches: vec![],
            total_matched: 0,
            metadata: QueryResponseMetadata {
                execution_duration_us: 0,
                snapshot_watermark: snapshot.watermark().0,
            },
        })
    }
}
```

Create `src/query/evaluators/mod.rs`:
```rust
pub mod hybrid;
pub mod neighborhood;
pub mod search;
pub mod temporal;

pub use hybrid::HybridEvaluator;
pub use neighborhood::NeighborhoodEvaluator;
pub use search::SearchEvaluator;
pub use temporal::TemporalEvaluator;
```

- [ ] **Step 4: Implement `KnowledgeQueryFacade` in `src/query/facade.rs`**

```rust
use crate::query::errors::*;
use crate::query::evaluators::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;
use arc_swap::ArcSwap;
use std::sync::Arc;

/// Single thread-safe read query composition facade over atomic projection snapshots.
#[derive(Debug)]
pub struct KnowledgeQueryFacade {
    snapshot: ArcSwap<ProjectionSnapshot>,
}

impl KnowledgeQueryFacade {
    /// Constructs a KnowledgeQueryFacade initialized with a projection snapshot.
    pub fn new(snapshot: Arc<ProjectionSnapshot>) -> Self {
        Self {
            snapshot: ArcSwap::new(snapshot),
        }
    }

    /// Atomically updates the active snapshot.
    pub fn update_snapshot(&self, new_snapshot: Arc<ProjectionSnapshot>) {
        self.snapshot.store(new_snapshot);
    }

    /// Obtains an immutable reference to the active projection snapshot.
    pub fn active_snapshot(&self) -> Arc<ProjectionSnapshot> {
        self.snapshot.load_full()
    }

    /// Evaluates a node neighborhood graph traversal query.
    pub fn query_neighborhood(&self, query: &NeighborhoodQuery) -> Result<QueryFacadeResult, QueryError> {
        let snapshot = self.active_snapshot();
        NeighborhoodEvaluator::evaluate(&snapshot, query)
    }

    /// Evaluates a point-in-time entity state query.
    pub fn query_point_in_time(&self, query: &PointInTimeQuery) -> Result<QueryFacadeResult, QueryError> {
        let snapshot = self.active_snapshot();
        TemporalEvaluator::evaluate(&snapshot, query)
    }

    /// Evaluates a lexical search query.
    pub fn query_search(&self, query: &LexicalSearchQuery) -> Result<QueryFacadeResult, QueryError> {
        let snapshot = self.active_snapshot();
        SearchEvaluator::evaluate(&snapshot, query)
    }

    /// Evaluates a compound hybrid query.
    pub fn query_hybrid(&self, query: &HybridSearchQuery) -> Result<QueryFacadeResult, QueryError> {
        let snapshot = self.active_snapshot();
        HybridEvaluator::evaluate(&snapshot, query)
    }
}
```

Update `crates/brain-services/src/query/mod.rs` to re-export `facade` and `evaluators`.

- [ ] **Step 5: Run test to verify PASS**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test knowledge_query_facade_tests
```
Expected: PASS cleanly.

- [ ] **Step 6: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): add KnowledgeQueryFacade and stateless evaluators"
```
