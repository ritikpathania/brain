# Phase 5.3.2 — Neighborhood Evaluator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 5.3.2 **`NeighborhoodEvaluator`** (`crates/brain-services/src/query/evaluators/neighborhood.rs`) executing deterministic BFS graph expansion up to `max_hops`, neighbor deduplication, metadata enrichment, temporal validity filtering, confidence thresholding, explicit sorting, and pagination slicing.

**Architecture:** Evaluator lives in `crates/brain-services/src/query/evaluators/neighborhood.rs`. `evaluate` reads `snapshot.graph()`, `snapshot.statistics()`, and `snapshot.temporal()`. Discovered outgoing and incoming neighbors are deduplicated and sorted by `KnowledgeEntityId` ASC before queuing. Canonical pipeline order: Candidate Discovery $\rightarrow$ Candidate Enrichment $\rightarrow$ Temporal Filter $\rightarrow$ Confidence Filter $\rightarrow$ Explicit Sort $\rightarrow$ Pagination.

**Tech Stack:** Rust (edition 2021), `brain-domain`, `uuid`.

## Global Constraints
- `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test neighborhood_evaluator_tests` must pass cleanly.
- `NeighborhoodEvaluator` MUST BE 100% stateless and MUST NOT mutate projection snapshots.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: Neighborhood Evaluator & Integration Tests | ✅ Completed | `9d82cf8` |
| **M1 Checkpoint** | **BFS Traversal & Contract Verification** | ✅ Completed | `9d82cf8` |

---

### Task 1: Neighborhood Evaluator & Integration Tests

**Files:**
- Modify: `crates/brain-services/src/query/evaluators/neighborhood.rs`
- Create: `crates/brain-services/tests/neighborhood_evaluator_tests.rs`

- [ ] **Step 1: Write failing integration test `crates/brain-services/tests/neighborhood_evaluator_tests.rs`**

```rust
use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::*;
use brain_domain::projection::search_index::*;
use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;
use brain_services::query::evaluators::*;
use brain_services::query::*;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

fn setup_test_snapshot() -> (KnowledgeEntityId, KnowledgeEntityId, KnowledgeEntityId, Arc<ProjectionSnapshot>) {
    let e_a = KnowledgeEntityId(Uuid::from_u128(10));
    let e_b = KnowledgeEntityId(Uuid::from_u128(20));
    let e_c = KnowledgeEntityId(Uuid::from_u128(30));

    let mut adj_reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp_reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats_reducer = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search_reducer = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    let now = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));
    let fact_id1 = FactVersionId(Uuid::new_v4());
    let assertion_id1 = AssertionId(Uuid::new_v4());

    let fact1 = FactVersion {
        id: fact_id1,
        assertion_id: assertion_id1,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(0.9).unwrap(),
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
        subject: e_a.clone(),
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Entity(e_b.clone()),
    };

    let event1 = FactEvent::FactRecorded {
        fact: fact1,
        assertion: Some(assertion1),
    };

    let _ = adj_reducer.apply_event(&event1);
    let _ = temp_reducer.apply_event(&event1);
    let _ = stats_reducer.apply_event(&event1);
    let _ = search_reducer.apply_event(&event1);

    let snapshot = ProjectionSnapshot::new(
        Arc::new(adj_reducer.state().clone()),
        Arc::new(temp_reducer.state().clone()),
        Arc::new(stats_reducer.state().clone()),
        Arc::new(search_reducer.state().clone()),
        Watermark(1),
    );

    (e_a, e_b, e_c, Arc::new(snapshot))
}

#[test]
fn test_neighborhood_max_hops_zero_and_traversal() {
    let (e_a, e_b, _, snapshot) = setup_test_snapshot();

    // max_hops = 0 -> returns only root_entity
    let query_zero = NeighborhoodQuery {
        root_entity: e_a.clone(),
        max_hops: 0,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res_zero = NeighborhoodEvaluator::evaluate(&snapshot, &query_zero).unwrap();
    assert_eq!(res_zero.total_matched, 1);
    assert_eq!(res_zero.matches[0].entity_id, e_a);

    // max_hops = 1 -> discovers e_a and e_b
    let query_one = NeighborhoodQuery {
        root_entity: e_a,
        max_hops: 1,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res_one = NeighborhoodEvaluator::evaluate(&snapshot, &query_one).unwrap();
    assert_eq!(res_one.total_matched, 2);
}

#[test]
fn test_neighborhood_missing_root_entity() {
    let (_, _, _, snapshot) = setup_test_snapshot();
    let missing_entity = KnowledgeEntityId(Uuid::from_u128(99999));

    let query = NeighborhoodQuery {
        root_entity: missing_entity,
        max_hops: 2,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res = NeighborhoodEvaluator::evaluate(&snapshot, &query).unwrap();
    assert_eq!(res.total_matched, 1);
    assert_eq!(res.matches[0].active_facts_count, 0);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test neighborhood_evaluator_tests
```
Expected: FAIL (unimplemented stub returning empty matches for non-zero max_hops).

- [ ] **Step 3: Implement BFS evaluation algorithm in `src/query/evaluators/neighborhood.rs`**

```rust
// crates/brain-services/src/query/evaluators/neighborhood.rs
use crate::query::errors::*;
use crate::query::filters::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::GraphNodeId;
use std::collections::{HashSet, VecDeque};

/// Stateless evaluator for node neighborhood graph traversal.
pub struct NeighborhoodEvaluator;

impl NeighborhoodEvaluator {
    /// Evaluates node neighborhood graph expansion against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        query: &NeighborhoodQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let mut visited: HashSet<KnowledgeEntityId> = HashSet::new();
        let mut queue: VecDeque<(KnowledgeEntityId, usize)> = VecDeque::new();
        let mut discovered: Vec<KnowledgeEntityId> = Vec::new();

        queue.push_back((query.root_entity.clone(), 0));
        visited.insert(query.root_entity.clone());

        while let Some((curr_entity, depth)) = queue.pop_front() {
            discovered.push(curr_entity.clone());

            if depth < query.max_hops {
                let node_id = GraphNodeId(EntityId(curr_entity.0));

                let mut neighbors = Vec::new();
                for edge_id in snapshot.graph().neighbors_out(&node_id) {
                    if let Some(edge) = snapshot.graph().edge(edge_id) {
                        neighbors.push(KnowledgeEntityId(edge.target.0 .0));
                    }
                }
                for edge_id in snapshot.graph().neighbors_in(&node_id) {
                    if let Some(edge) = snapshot.graph().edge(edge_id) {
                        neighbors.push(KnowledgeEntityId(edge.source.0 .0));
                    }
                }

                neighbors.sort_by(|a, b| a.0.cmp(&b.0));
                neighbors.dedup();

                for neighbor in neighbors {
                    if visited.insert(neighbor.clone()) {
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }
        }

        let mut candidates = Vec::with_capacity(discovered.len());
        for entity_id in discovered {
            let node_id = GraphNodeId(EntityId(entity_id.0));
            let degree = snapshot.graph().degree(&node_id);
            let stats = snapshot.statistics().get(&entity_id);

            let active_facts_count = stats.map_or(0, |s| s.active_facts_count);
            let average_confidence = stats.map_or(
                Confidence::new(0.0).unwrap(),
                |s| s.average_confidence(),
            );

            let satisfies_temporal = match query.temporal_mode {
                TemporalMode::CurrentActive => active_facts_count > 0 || stats.is_none(),
                TemporalMode::ValidAt(at_ts) => !snapshot.temporal().facts_at(&entity_id, at_ts).is_empty(),
                TemporalMode::AllHistorical => true,
            };

            if satisfies_temporal {
                candidates.push(EntityMatch {
                    entity_id,
                    active_facts_count,
                    average_confidence,
                    graph_metadata: Some(GraphMetadata {
                        in_degree: degree.in_degree,
                        out_degree: degree.out_degree,
                    }),
                    search_metadata: None,
                });
            }
        }

        filter_by_confidence(&mut candidates, query.confidence_filter.as_ref());

        let default_ordering = QueryOrdering {
            field: SortField::Confidence,
            direction: SortDirection::Descending,
        };
        sort_matches(&mut candidates, Some(&default_ordering));
        let (paginated, total_matched) = paginate_matches(&candidates, &query.pagination);

        Ok(QueryFacadeResult {
            matches: paginated,
            total_matched,
            metadata: QueryResponseMetadata {
                execution_duration_us: 0,
                snapshot_watermark: snapshot.watermark().0,
            },
        })
    }
}
```

- [ ] **Step 4: Run test to verify PASS**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test neighborhood_evaluator_tests
```
Expected: PASS cleanly.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement NeighborhoodEvaluator BFS graph expansion and pipeline filtering"
```
