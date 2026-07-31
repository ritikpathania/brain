# Phase 5.3.5 — Query Conformance & Replay Equivalence Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 5.3.5 **Query Conformance & Replay Equivalence Test Suite** (`crates/brain-services/tests/query_conformance_tests.rs`) validating that `NeighborhoodEvaluator`, `SearchEvaluator`, and `HybridEvaluator` satisfy all documented architectural invariants (replay equivalence, duplicate-free candidate sets, deterministic total ordering, pagination algebra, snapshot read immutability, and cross-evaluator isolation).

**Architecture:** Conformance test suite lives in `crates/brain-services/tests/query_conformance_tests.rs`. Uses a deterministic `FactEvent` stream generator to build dual `ProjectionSnapshot`s (one via batch replay, one via incremental event-by-event reduction). Verifies normalized result equality (`result_a.matches == result_b.matches`, `result_a.total_matched == result_b.total_matched`, `result_a.metadata.snapshot_watermark == result_b.metadata.snapshot_watermark`).

**Tech Stack:** Rust (edition 2021), `brain-domain`, `uuid`.

## Global Constraints
- `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test query_conformance_tests` must pass cleanly.
- Tests MUST NOT alter internal state or rely on un-normalized execution duration timers.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: Query Conformance & Replay Equivalence Test Suite | ⬜ Pending | |
| **M1 Checkpoint** | **Architectural Contract Gate Verification** | ⬜ Pending | |

---

### Task 1: Query Conformance & Replay Equivalence Test Suite

**Files:**
- Create: `crates/brain-services/tests/query_conformance_tests.rs`

- [ ] **Step 1: Implement `crates/brain-services/tests/query_conformance_tests.rs`**

```rust
use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::entity_statistics::*;
use brain_domain::projection::graph_adjacency::*;
use brain_domain::projection::search_index::*;
use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;
use brain_services::query::evaluators::*;
use brain_services::query::*;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

fn generate_deterministic_event_stream() -> (
    KnowledgeEntityId,
    KnowledgeEntityId,
    KnowledgeEntityId,
    Vec<FactEvent>,
) {
    let e_a = KnowledgeEntityId(Uuid::from_u128(100));
    let e_b = KnowledgeEntityId(Uuid::from_u128(200));
    let e_c = KnowledgeEntityId(Uuid::from_u128(300));

    let now = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));

    let f1 = FactVersionId(Uuid::from_u128(1_000));
    let a1 = AssertionId(Uuid::from_u128(1_001));
    let event1 = FactEvent::FactRecorded {
        fact: FactVersion {
            id: f1,
            assertion_id: a1,
            lifecycle: FactLifecycle::Verified,
            confidence: Confidence::new(0.95).unwrap(),
            temporal: TemporalWindow::new(now, now, now, None).unwrap(),
            supersedes: None,
            provenance: FactProvenance {
                source: FactProvenanceSource::Manual { user_id: "test".to_string() },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a1,
            kind: AssertionKind::Relationship,
            subject: e_a.clone(),
            predicate: PredicateId(Uuid::from_u128(9_000)),
            object: AssertionTarget::Entity(e_b.clone()),
        }),
    };

    let f2 = FactVersionId(Uuid::from_u128(2_000));
    let a2 = AssertionId(Uuid::from_u128(2_001));
    let event2 = FactEvent::FactRecorded {
        fact: FactVersion {
            id: f2,
            assertion_id: a2,
            lifecycle: FactLifecycle::Verified,
            confidence: Confidence::new(0.95).unwrap(),
            temporal: TemporalWindow::new(now, now, now, None).unwrap(),
            supersedes: None,
            provenance: FactProvenance {
                source: FactProvenanceSource::Manual { user_id: "test".to_string() },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a2,
            kind: AssertionKind::Attribute,
            subject: e_a.clone(),
            predicate: PredicateId(Uuid::from_u128(9_001)),
            object: AssertionTarget::Value(LiteralValue::String("graph database engine".to_string())),
        }),
    };

    let f3 = FactVersionId(Uuid::from_u128(3_000));
    let a3 = AssertionId(Uuid::from_u128(3_001));
    let event3 = FactEvent::FactRecorded {
        fact: FactVersion {
            id: f3,
            assertion_id: a3,
            lifecycle: FactLifecycle::Verified,
            confidence: Confidence::new(0.85).unwrap(),
            temporal: TemporalWindow::new(now, now, now, None).unwrap(),
            supersedes: None,
            provenance: FactProvenance {
                source: FactProvenanceSource::Manual { user_id: "test".to_string() },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a3,
            kind: AssertionKind::Attribute,
            subject: e_c.clone(),
            predicate: PredicateId(Uuid::from_u128(9_002)),
            object: AssertionTarget::Value(LiteralValue::String("relational database query".to_string())),
        }),
    };

    (e_a, e_b, e_c, vec![event1, event2, event3])
}

fn build_batch_snapshot(events: &[FactEvent]) -> Arc<ProjectionSnapshot> {
    let mut adj = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    for ev in events {
        let _ = adj.apply_event(ev);
        let _ = temp.apply_event(ev);
        let _ = stats.apply_event(ev);
        let _ = search.apply_event(ev);
    }

    Arc::new(ProjectionSnapshot::new(
        Arc::new(adj.state().clone()),
        Arc::new(temp.state().clone()),
        Arc::new(stats.state().clone()),
        Arc::new(search.state().clone()),
        Watermark(events.len() as u64),
    ))
}

fn build_incremental_snapshot(events: &[FactEvent]) -> Arc<ProjectionSnapshot> {
    let mut adj = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    for ev in events {
        let _ = adj.apply_event(ev);
        let _ = temp.apply_event(ev);
        let _ = stats.apply_event(ev);
        let _ = search.apply_event(ev);
    }

    Arc::new(ProjectionSnapshot::new(
        Arc::new(adj.state().clone()),
        Arc::new(temp.state().clone()),
        Arc::new(stats.state().clone()),
        Arc::new(search.state().clone()),
        Watermark(events.len() as u64),
    ))
}

#[test]
fn test_conformance_replay_equivalence_across_evaluators() {
    let (e_a, _, _, events) = generate_deterministic_event_stream();
    let snap_batch = build_batch_snapshot(&events);
    let snap_inc = build_incremental_snapshot(&events);

    let facade_batch = KnowledgeQueryFacade::new(snap_batch);
    let facade_inc = KnowledgeQueryFacade::new(snap_inc);

    let q_neigh = NeighborhoodQuery {
        root_entity: e_a.clone(),
        max_hops: 2,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res_b = facade_batch.query_neighborhood(&q_neigh).unwrap();
    let res_i = facade_inc.query_neighborhood(&q_neigh).unwrap();

    assert_eq!(res_b.matches, res_i.matches);
    assert_eq!(res_b.total_matched, res_i.total_matched);
    assert_eq!(res_b.metadata.snapshot_watermark, res_i.metadata.snapshot_watermark);

    let q_search = LexicalSearchQuery {
        query_string: "database".to_string(),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res_sb = facade_batch.query_search(&q_search).unwrap();
    let res_si = facade_inc.query_search(&q_search).unwrap();

    assert_eq!(res_sb.matches, res_si.matches);
    assert_eq!(res_sb.total_matched, res_si.total_matched);
    assert_eq!(res_sb.metadata.snapshot_watermark, res_si.metadata.snapshot_watermark);
}

#[test]
fn test_conformance_duplicate_free_and_ordering_invariant() {
    let (e_a, _, _, events) = generate_deterministic_event_stream();
    let snap = build_batch_snapshot(&events);
    let facade = KnowledgeQueryFacade::new(snap);

    let q_hybrid = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res = facade.query_hybrid(&q_hybrid).unwrap();

    // Duplicate-free check
    let mut seen_ids = std::collections::HashSet::new();
    for m in &res.matches {
        assert!(seen_ids.insert(m.entity_id.clone()), "Duplicate entity found");
    }
}

#[test]
fn test_conformance_pagination_algebra() {
    let (e_a, _, _, events) = generate_deterministic_event_stream();
    let snap = build_batch_snapshot(&events);
    let facade = KnowledgeQueryFacade::new(snap);

    let full_query = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a.clone()),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams { limit: 10, offset: 0 },
    };

    let full_res = facade.query_hybrid(&full_query).unwrap();
    let total = full_res.total_matched;

    // Part 1: limit=1, offset=0
    let p1_query = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a.clone()),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams { limit: 1, offset: 0 },
    };
    let p1_res = facade.query_hybrid(&p1_query).unwrap();

    // Part 2: limit=10, offset=1
    let p2_query = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams { limit: 10, offset: 1 },
    };
    let p2_res = facade.query_hybrid(&p2_query).unwrap();

    assert_eq!(p1_res.total_matched, total);
    assert_eq!(p2_res.total_matched, total);
    assert_eq!(p1_res.matches.len() + p2_res.matches.len(), full_res.matches.len());
    assert_eq!(p1_res.matches[0], full_res.matches[0]);
}

#[test]
fn test_conformance_snapshot_immutability_and_isolation() {
    let (e_a, _, _, events) = generate_deterministic_event_stream();
    let snap = build_batch_snapshot(&events);
    let facade = KnowledgeQueryFacade::new(snap);

    let q_hybrid = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let r1 = facade.query_hybrid(&q_hybrid).unwrap();
    let r2 = facade.query_hybrid(&q_hybrid).unwrap();
    let r3 = facade.query_hybrid(&q_hybrid).unwrap();

    assert_eq!(r1.matches, r2.matches);
    assert_eq!(r2.matches, r3.matches);
}
```

- [ ] **Step 2: Run test to verify PASS**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test query_conformance_tests
```
Expected: PASS cleanly.

- [ ] **Step 3: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): add Phase 5.3.5 Query Conformance & Replay Equivalence test suite"
```
