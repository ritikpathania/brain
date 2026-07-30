# Phase 5.2 — KnowledgeQueryFacade & Projection Snapshot Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-services` (`src/query/`)

---

## 1. Executive Summary & Architectural Invariants

Phase 5.2 constructs the **`KnowledgeQueryFacade`** and atomic **`ProjectionSnapshot`** container in `brain-services::query`. It establishes a single, thread-safe composition read point over all four Phase 4 read models (`GraphAdjacencyState`, `TemporalState`, `EntityStatisticsState`, and `SearchIndexState`).

### Core Architectural Invariants:
1. **Atomic Cross-Projection Snapshot**: A single immutable `ProjectionSnapshot` owns encapsulated `Arc` references to all 4 read models and the snapshot's watermark (`Watermark`), ensuring 100% watermark consistency across all queries.
2. **Lock-Free Zero-Copy Read Operations**: `KnowledgeQueryFacade` maintains an `ArcSwap<ProjectionSnapshot>`, enabling zero-copy reader loads without reader-writer lock contention.
3. **Publication Immutability**: `ProjectionSnapshot` instances are strictly immutable after publication. Catch-up updates build a new `ProjectionSnapshot` and publish via atomic `ArcSwap::store(...)`.
4. **Stateless Query Evaluators**: Query evaluation logic is decoupled into stateless evaluator functions in `src/query/evaluators/`, taking `(&ProjectionSnapshot, &Query)` and returning `Result<QueryFacadeResult, QueryError>`.

---

## 2. Component Layout & Module Structure

```text
crates/brain-services/src/query/
├── mod.rs
├── models.rs                <-- Phase 5.1 Request/Response DTOs
├── errors.rs                <-- Phase 5.1 QueryError hierarchy
├── snapshot.rs              <-- Phase 5.2 ProjectionSnapshot container
├── facade.rs                <-- Phase 5.2 KnowledgeQueryFacade entrypoint
└── evaluators/
    ├── mod.rs               <-- Phase 5.2 Evaluator re-exports
    ├── neighborhood.rs      <-- Phase 5.2 NeighborhoodEvaluator
    ├── temporal.rs          <-- Phase 5.2 TemporalEvaluator
    ├── search.rs            <-- Phase 5.2 SearchEvaluator
    └── hybrid.rs            <-- Phase 5.2 HybridEvaluator
```

---

## 3. ProjectionSnapshot Container (`src/query/snapshot.rs`)

```rust
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

---

## 4. KnowledgeQueryFacade Entrypoint (`src/query/facade.rs`)

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

---

## 5. Verification & Testing Strategy

1. **Unit & Facade Tests (`crates/brain-services/tests/knowledge_query_facade_tests.rs`)**:
   - Verifies `ArcSwap` atomic snapshot updates (`update_snapshot`).
   - Verifies zero-copy snapshot loads (`active_snapshot`).
   - Verifies evaluator delegation for neighborhood, temporal, search, and hybrid queries.
   - Verifies concurrent reader/writer safety: reader A holding snapshot X remains consistent when writer publishes snapshot Y, while reader B reads Y.
