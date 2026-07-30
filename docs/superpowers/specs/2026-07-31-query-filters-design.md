# Phase 5.3.1 — Query Processing Primitives Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-services` (`src/query/filters/`)

---

## 1. Executive Summary & Architectural Invariants

Phase 5.3.1 establishes the pure, reusable **Query Processing Primitives** (`brain-services::query::filters`). These primitives provide pure, projection-independent helper functions for filtering, sorting, and paginating candidates across all query evaluators (`NeighborhoodEvaluator`, `SearchEvaluator`, `TemporalEvaluator`, and `HybridEvaluator`).

### Core Architectural Invariants:
1. **Purity & Projection Independence**: Functions in `src/query/filters/` are 100% deterministic and pure. They have zero dependency on `ProjectionSnapshot` or projection states, operating on domain types (`EntityMatch`, `Confidence`, `KnowledgeEntityId`).
2. **Normative Pipeline Ordering**: Every evaluator enforces a consistent evaluation sequence:
   ```text
   Candidate Retrieval ──► Temporal Filter ──► Confidence Filter ──► Deterministic Sort ──► Pagination
   ```
3. **Deterministic Total Ordering**: Sorting uses stable sorting (`sort_by`) and guarantees 100% total ordering by appending `KnowledgeEntityId` ASC as a secondary/tertiary tie-breaker whenever primary sort keys compare equal.
4. **Order Preservation**: Filtering operations (`filter_by_confidence`) strictly preserve the relative input ordering of retained candidates.
5. **Strict Temporal Interval Semantics**: Validity checks evaluate half-open intervals `[valid_from, valid_until)` (inclusive `valid_from`, exclusive `valid_until`, `None` = unbounded future).
6. **Strict Pagination Semantics**: Pagination slicing occurs strictly AFTER sorting. Invalid or out-of-bounds offsets (`offset > total`) return empty slices without mutating `total_matched`.

---

## 2. Component Layout & Module Structure

```text
crates/brain-services/src/query/filters/
├── mod.rs           <-- Re-exports and public filter API
├── temporal.rs      <-- TemporalMode evaluation helpers
├── confidence.rs    <-- ConfidenceFilter thresholding
├── ordering.rs      <-- Deterministic sorting with tie-breaking
└── pagination.rs    <-- Safe offset/limit candidate slicing
```

---

## 3. Module Specifications & Interfaces

### 1. `confidence.rs` — Confidence Thresholding
```rust
use crate::query::models::{ConfidenceFilter, EntityMatch};

/// Filters candidates against a ConfidenceFilter threshold, preserving relative candidate ordering.
pub fn filter_by_confidence(candidates: &mut Vec<EntityMatch>, filter: Option<&ConfidenceFilter>) {
    if let Some(conf_filter) = filter {
        let min_val = conf_filter.min_confidence.value();
        candidates.retain(|item| item.average_confidence.value() >= min_val);
    }
}
```

### 2. `temporal.rs` — Temporal Validity Helpers
```rust
use brain_domain::bkf::Timestamp;

/// Evaluates if a half-open validity interval `[valid_from, valid_until)` satisfies a target `Timestamp` query.
/// Inclusive lower bound (`valid_from <= target_time`), exclusive upper bound (`valid_until > target_time`), `None` = unbounded.
pub fn is_valid_at(valid_from: Timestamp, valid_until: Option<Timestamp>, target_time: Timestamp) -> bool {
    if valid_from > target_time {
        return false;
    }
    match valid_until {
        None => true,
        Some(until) => until > target_time,
    }
}
```

### 3. `ordering.rs` — Deterministic Sorting with Tie-Breaking
```rust
use crate::query::models::{EntityMatch, QueryOrdering, SortDirection, SortField};

/// Performs in-place stable sorting over candidates with deterministic EntityId ASC tie-breaking.
pub fn sort_matches(candidates: &mut [EntityMatch], ordering: Option<&QueryOrdering>) {
    let ordering = match ordering {
        Some(ord) => ord,
        None => return,
    };

    candidates.sort_by(|a, b| {
        let primary_cmp = match ordering.field {
            SortField::Confidence => {
                let a_val = a.average_confidence.value();
                let b_val = b.average_confidence.value();
                a_val.total_cmp(&b_val)
            }
            SortField::Degree => {
                let a_deg = a.graph_metadata.as_ref().map_or(0, |g| g.in_degree + g.out_degree);
                let b_deg = b.graph_metadata.as_ref().map_or(0, |g| g.in_degree + g.out_degree);
                a_deg.cmp(&b_deg)
            }
            SortField::Recency => {
                // Placeholder: Recency ordering will be populated in 5.3.2 once evaluator metadata exposes timestamps.
                std::cmp::Ordering::Equal
            }
        };

        let primary_cmp = match ordering.direction {
            SortDirection::Ascending => primary_cmp,
            SortDirection::Descending => primary_cmp.reverse(),
        };

        if primary_cmp != std::cmp::Ordering::Equal {
            primary_cmp
        } else {
            // Deterministic tie-breaker: EntityId ASC
            a.entity_id.0.cmp(&b.entity_id.0)
        }
    });
}
```

### 4. `pagination.rs` — Safe Candidate Slicing
```rust
use crate::query::models::{EntityMatch, PaginationParams};

/// Applies limit and offset slicing to candidates, returning the total matched count before pagination.
pub fn paginate_matches(candidates: Vec<EntityMatch>, pagination: &PaginationParams) -> (Vec<EntityMatch>, usize) {
    let total_matched = candidates.len();
    if pagination.limit == 0 || pagination.offset >= total_matched {
        return (vec![], total_matched);
    }

    let end = pagination.offset.saturating_add(pagination.limit).min(total_matched);
    let paginated = candidates[pagination.offset..end].to_vec();
    (paginated, total_matched)
}
```

---

## 4. Verification & Testing Strategy

1. **Unit Tests (`crates/brain-services/tests/query_filters_tests.rs`)**:
   - `test_filter_by_confidence_threshold_order_preservation`: Verifies retains items $\ge$ min_confidence while preserving candidate input ordering.
   - `test_is_valid_at_half_open_interval`: Verifies half-open `[valid_from, valid_until)` interval validity at timestamp.
   - `test_sort_matches_deterministic_tie_breaking`: Verifies primary field sorting and secondary `EntityId` ASC tie-breaking when primary sort keys compare equal.
   - `test_paginate_matches_boundary_conditions`: Verifies `offset > total`, `limit == 0`, and normal slicing behavior.
