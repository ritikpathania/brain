# Phase 5.3.1 — Query Processing Primitives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 5.3.1 **Query Processing Primitives** (`crates/brain-services/src/query/filters/`) establishing pure, projection-independent helper functions for confidence thresholding, half-open temporal validity checks, stable deterministic sorting with tie-breaking, and safe pagination slicing.

**Architecture:** Helpers live in `crates/brain-services/src/query/filters/` (`confidence.rs`, `temporal.rs`, `ordering.rs`, `pagination.rs`, `mod.rs`). Pure functions operate on BKF domain models (`EntityMatch`, `Confidence`, `Timestamp`, `KnowledgeEntityId`). `sort_matches` applies `total_cmp` and appends `KnowledgeEntityId` ASC as a deterministic tie-breaker. `filter_by_confidence` preserves candidate input order. `paginate_matches` performs post-sort slicing with saturating arithmetic.

**Tech Stack:** Rust (edition 2021), `serde`, `uuid`.

## Global Constraints
- `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test query_filters_tests` must pass cleanly.
- Functions in `filters` MUST BE 100% pure and MUST NOT depend on `ProjectionSnapshot` or projection state types.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: Reusable Query Processing Primitives & Unit Tests | ✅ Completed | `ef9677d` |
| **M1 Checkpoint** | **Unit & Contract Verification** | ✅ Completed | `ef9677d` |

---

### Task 1: Reusable Query Processing Primitives & Unit Tests

**Files:**
- Create: `crates/brain-services/src/query/filters/mod.rs`
- Create: `crates/brain-services/src/query/filters/confidence.rs`
- Create: `crates/brain-services/src/query/filters/temporal.rs`
- Create: `crates/brain-services/src/query/filters/ordering.rs`
- Create: `crates/brain-services/src/query/filters/pagination.rs`
- Modify: `crates/brain-services/src/query/mod.rs`
- Create: `crates/brain-services/tests/query_filters_tests.rs`

- [ ] **Step 1: Write failing unit test `crates/brain-services/tests/query_filters_tests.rs`**

```rust
use brain_domain::bkf::*;
use brain_services::query::filters::*;
use brain_services::query::*;
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

#[test]
fn test_filter_by_confidence_threshold_order_preservation() {
    let e1 = KnowledgeEntityId(Uuid::from_u128(1));
    let e2 = KnowledgeEntityId(Uuid::from_u128(2));
    let e3 = KnowledgeEntityId(Uuid::from_u128(3));

    let mut candidates = vec![
        EntityMatch {
            entity_id: e1.clone(),
            active_facts_count: 1,
            average_confidence: Confidence::new(0.9).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
        EntityMatch {
            entity_id: e2.clone(),
            active_facts_count: 1,
            average_confidence: Confidence::new(0.5).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
        EntityMatch {
            entity_id: e3.clone(),
            active_facts_count: 1,
            average_confidence: Confidence::new(0.85).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
    ];

    let filter = ConfidenceFilter {
        min_confidence: Confidence::new(0.8).unwrap(),
    };

    filter_by_confidence(&mut candidates, Some(&filter));

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].entity_id, e1);
    assert_eq!(candidates[1].entity_id, e3);
}

#[test]
fn test_is_valid_at_half_open_interval() {
    let t10 = Timestamp(UNIX_EPOCH + Duration::from_secs(10));
    let t20 = Timestamp(UNIX_EPOCH + Duration::from_secs(20));
    let t30 = Timestamp(UNIX_EPOCH + Duration::from_secs(30));

    // Valid: [10, 30) at t20
    assert!(is_valid_at(t10, Some(t30), t20));
    // Valid: inclusive lower bound [10, 30) at t10
    assert!(is_valid_at(t10, Some(t30), t10));
    // Invalid: exclusive upper bound [10, 30) at t30
    assert!(!is_valid_at(t10, Some(t30), t30));
    // Valid: open upper bound [10, None) at t30
    assert!(is_valid_at(t10, None, t30));
}

#[test]
fn test_sort_matches_deterministic_tie_breaking() {
    let uuid_b = Uuid::from_u128(20);
    let uuid_a = Uuid::from_u128(10);
    let uuid_c = Uuid::from_u128(30);

    let e_b = KnowledgeEntityId(uuid_b);
    let e_a = KnowledgeEntityId(uuid_a);
    let e_c = KnowledgeEntityId(uuid_c);

    let mut candidates = vec![
        EntityMatch {
            entity_id: e_b.clone(),
            active_facts_count: 1,
            average_confidence: Confidence::new(0.9).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
        EntityMatch {
            entity_id: e_a.clone(),
            active_facts_count: 1,
            average_confidence: Confidence::new(0.9).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
        EntityMatch {
            entity_id: e_c.clone(),
            active_facts_count: 1,
            average_confidence: Confidence::new(0.9).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
    ];

    let ordering = QueryOrdering {
        field: SortField::Confidence,
        direction: SortDirection::Descending,
    };

    sort_matches(&mut candidates, Some(&ordering));

    // Primary keys equal (0.9), secondary tie-breaker EntityId ASC (uuid_a, uuid_b, uuid_c)
    assert_eq!(candidates[0].entity_id, e_a);
    assert_eq!(candidates[1].entity_id, e_b);
    assert_eq!(candidates[2].entity_id, e_c);
}

#[test]
fn test_paginate_matches_boundary_conditions() {
    let e1 = KnowledgeEntityId(Uuid::from_u128(1));
    let e2 = KnowledgeEntityId(Uuid::from_u128(2));

    let candidates = vec![
        EntityMatch {
            entity_id: e1.clone(),
            active_facts_count: 1,
            average_confidence: Confidence::new(0.9).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
        EntityMatch {
            entity_id: e2.clone(),
            active_facts_count: 1,
            average_confidence: Confidence::new(0.8).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
    ];

    let (res, total) = paginate_matches(candidates.clone(), &PaginationParams { limit: 1, offset: 0 });
    assert_eq!(total, 2);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].entity_id, e1);

    // Offset out of bounds
    let (res_oob, total_oob) = paginate_matches(candidates, &PaginationParams { limit: 10, offset: 5 });
    assert_eq!(total_oob, 2);
    assert!(res_oob.is_empty());
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test query_filters_tests
```
Expected: FAIL (unresolved import `brain_services::query::filters`).

- [ ] **Step 3: Implement `filters/` modules and re-export in `query/mod.rs`**

Create `src/query/filters/confidence.rs`:
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

Create `src/query/filters/temporal.rs`:
```rust
use brain_domain::bkf::Timestamp;

/// Evaluates if a half-open validity interval `[valid_from, valid_until)` satisfies a target `Timestamp` query.
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

Create `src/query/filters/ordering.rs`:
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
            SortField::Recency => std::cmp::Ordering::Equal,
        };

        let primary_cmp = match ordering.direction {
            SortDirection::Ascending => primary_cmp,
            SortDirection::Descending => primary_cmp.reverse(),
        };

        if primary_cmp != std::cmp::Ordering::Equal {
            primary_cmp
        } else {
            a.entity_id.0.cmp(&b.entity_id.0)
        }
    });
}
```

Create `src/query/filters/pagination.rs`:
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

Create `src/query/filters/mod.rs`:
```rust
pub mod confidence;
pub mod ordering;
pub mod pagination;
pub mod temporal;

pub use confidence::filter_by_confidence;
pub use ordering::sort_matches;
pub use pagination::paginate_matches;
pub use temporal::is_valid_at;
```

Update `crates/brain-services/src/query/mod.rs` to re-export `filters`.

- [ ] **Step 4: Run test to verify PASS**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test query_filters_tests
```
Expected: PASS cleanly.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): add Phase 5.3.1 Query Processing Primitives (filtering, ordering, pagination)"
```
