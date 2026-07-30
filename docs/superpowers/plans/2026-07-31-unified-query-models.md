# Phase 5.1 — Unified Query Models Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 5.1 **Unified Query Models** (`crates/brain-services/src/query/`) establishing request models, response models, filtering primitives, pagination, ordering, and error hierarchy with zero dependency on projection internals.

**Architecture:** Models live in `crates/brain-services/src/query/` (`models.rs`, `errors.rs`, `mod.rs`). `TemporalMode` provides mutually exclusive filtering (`CurrentActive`, `ValidAt`, `AllHistorical`). `ConfidenceFilter` leverages BKF domain `Confidence`. `PaginationParams` and `QueryOrdering` decouple pagination and sorting. `EntityMatch` encapsulates optional `GraphMetadata` and `SearchMetadata`. `QueryResponseMetadata` isolates execution timing and snapshot watermarks.

**Tech Stack:** Rust (edition 2021), `serde`, `thiserror`, `uuid`.

## Global Constraints
- `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test query_models_tests` must pass cleanly.
- `SearchToken` or projection internal types MUST NOT be exposed in service query response models.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: Unified Query Models & Error Hierarchy | ⬜ Pending | |
| **M1 Checkpoint** | **Unit & Contract Verification** | ⬜ Pending | |

---

### Task 1: Unified Query Models & Error Hierarchy

**Files:**
- Create: `crates/brain-services/src/query/mod.rs`
- Create: `crates/brain-services/src/query/models.rs`
- Create: `crates/brain-services/src/query/errors.rs`
- Modify: `crates/brain-services/src/lib.rs` (re-export `query` module)
- Create: `crates/brain-services/tests/query_models_tests.rs`

- [ ] **Step 1: Write failing unit test in `crates/brain-services/tests/query_models_tests.rs`**

```rust
use brain_domain::bkf::*;
use brain_services::query::*;
use uuid::Uuid;

#[test]
fn test_query_models_and_defaults() {
    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let now = Timestamp::now();

    let query = HybridSearchQuery {
        query_string: "rust graph".to_string(),
        root_entity: Some(entity_id.clone()),
        temporal_mode: TemporalMode::ValidAt(now),
        confidence_filter: Some(ConfidenceFilter {
            min_confidence: Confidence::new(0.8).unwrap(),
        }),
        ordering: Some(QueryOrdering {
            field: SortField::Confidence,
            direction: SortDirection::Descending,
        }),
        pagination: PaginationParams::default(),
    };

    assert_eq!(query.pagination.limit, 50);
    assert_eq!(query.pagination.offset, 0);

    let match_item = EntityMatch {
        entity_id,
        active_facts_count: 5,
        average_confidence: Confidence::new(0.95).unwrap(),
        graph_metadata: Some(GraphMetadata {
            in_degree: 3,
            out_degree: 2,
        }),
        search_metadata: Some(SearchMetadata {
            matched_terms: vec!["rust".to_string(), "graph".to_string()],
        }),
    };

    let result = QueryResult {
        matches: vec![match_item],
        total_matched: 1,
        metadata: QueryResponseMetadata {
            execution_duration_us: 120,
            snapshot_watermark: 42,
        },
    };

    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.metadata.snapshot_watermark, 42);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test query_models_tests
```
Expected: FAIL (unresolved import `brain_services::query`).

- [ ] **Step 3: Implement `errors.rs`, `models.rs`, `mod.rs`, and export in `lib.rs`**

Write `errors.rs`:
```rust
use thiserror::Error;

/// Strongly-typed query errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum QueryError {
    #[error("Entity not found: {0}")]
    EntityNotFound(String),
    #[error("Invalid query parameters: {0}")]
    InvalidParameters(String),
    #[error("Query timeout exceeded after {0} ms")]
    Timeout(u64),
    #[error("Unsupported query capability: {0}")]
    UnsupportedQuery(String),
    #[error("Query evaluation failed: {0}")]
    EvaluationFailed(String),
}
```

Write `models.rs`:
```rust
use brain_domain::bkf::*;
use serde::{Deserialize, Serialize};

/// Mutually exclusive temporal validity filtering mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalMode {
    CurrentActive,
    ValidAt(Timestamp),
    AllHistorical,
}

/// Confidence score threshold filter using BKF Confidence value object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceFilter {
    pub min_confidence: Confidence,
}

/// Sort field options for query ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortField {
    Confidence,
    Degree,
    Recency,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Query ordering configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryOrdering {
    pub field: SortField,
    pub direction: SortDirection,
}

/// Reusable pagination parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaginationParams {
    pub limit: usize,
    pub offset: usize,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self { limit: 50, offset: 0 }
    }
}

/// Parameters for node neighborhood graph traversal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeighborhoodQuery {
    pub root_entity: KnowledgeEntityId,
    pub max_hops: usize,
    pub temporal_mode: TemporalMode,
    pub confidence_filter: Option<ConfidenceFilter>,
    pub pagination: PaginationParams,
}

/// Parameters for point-in-time entity state lookups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointInTimeQuery {
    pub entity: KnowledgeEntityId,
    pub timestamp: Timestamp,
}

/// Parameters for lexical inverted search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LexicalSearchQuery {
    pub query_string: String,
    pub temporal_mode: TemporalMode,
    pub confidence_filter: Option<ConfidenceFilter>,
    pub pagination: PaginationParams,
}

/// Compound hybrid query combining search, time, metrics, and graph topology.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HybridSearchQuery {
    pub query_string: String,
    pub root_entity: Option<KnowledgeEntityId>,
    pub temporal_mode: TemporalMode,
    pub confidence_filter: Option<ConfidenceFilter>,
    pub ordering: Option<QueryOrdering>,
    pub pagination: PaginationParams,
}

/// Graph topology metadata for matched entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphMetadata {
    pub in_degree: usize,
    pub out_degree: usize,
}

/// Search lexical match metadata (uses String terms, keeping SearchToken internal).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMetadata {
    pub matched_terms: Vec<String>,
}

/// Core matched entity result item with optional specialized metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityMatch {
    pub entity_id: KnowledgeEntityId,
    pub active_facts_count: usize,
    pub average_confidence: Confidence,
    pub graph_metadata: Option<GraphMetadata>,
    pub search_metadata: Option<SearchMetadata>,
}

/// Execution metrics container separate from semantic result items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResponseMetadata {
    pub execution_duration_us: u64,
    pub snapshot_watermark: u64,
}

/// Unified query result wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    pub matches: Vec<EntityMatch>,
    pub total_matched: usize,
    pub metadata: QueryResponseMetadata,
}
```

Write `mod.rs`:
```rust
pub mod errors;
pub mod models;

pub use errors::*;
pub use models::*;
```

Update `crates/brain-services/src/lib.rs` to add `pub mod query;`.

- [ ] **Step 4: Run test to verify PASS**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test query_models_tests
```
Expected: PASS cleanly.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): add Phase 5.1 Unified Query Models and QueryError hierarchy"
```
