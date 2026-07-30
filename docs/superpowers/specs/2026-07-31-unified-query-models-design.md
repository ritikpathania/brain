# Phase 5.1 — Unified Query Models Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-services` (`src/query/`)

---

## 1. Executive Summary & Architectural Boundaries

Phase 5.1 defines the pure, strongly-typed public query language and error model for the service layer in `brain-services::query`. It establishes request models, response models, filtering primitives, pagination, ordering, and errors with **zero dependency on projection internals** and **no leakage of projection-specific value objects** (such as `SearchToken`).

---

## 2. Filtering, Ordering, & Pagination Primitives (`src/query/models.rs`)

### 1. Mutually Exclusive Temporal Filter
```rust
use brain_domain::bkf::*;
use serde::{Deserialize, Serialize};

/// Mutually exclusive temporal validity filtering mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalMode {
    /// Returns only currently active facts (valid_until is None).
    CurrentActive,
    /// Returns facts that were valid at the specified historical timestamp.
    ValidAt(Timestamp),
    /// Includes all historical and active facts without temporal filtering.
    AllHistorical,
}
```

### 2. Domain-Enforced Confidence Filter
```rust
/// Confidence score threshold filter using BKF Confidence value object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceFilter {
    /// Minimum confidence threshold enforced by BKF Confidence invariant [0.0..1.0].
    pub min_confidence: Confidence,
}
```

### 3. Decoupled Ordering & Pagination
```rust
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
```

---

## 3. Dedicated Request Models (`src/query/models.rs`)

```rust
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
```

---

## 4. Response Models & Metadata (`src/query/models.rs`)

```rust
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

---

## 5. Strongly-Typed Errors (`src/query/errors.rs`)

```rust
/// Strongly-typed query errors.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
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
