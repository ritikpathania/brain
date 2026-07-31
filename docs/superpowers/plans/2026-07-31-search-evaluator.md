# Phase 5.3.3 — Lexical Search Evaluator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 5.3.3 **`SearchEvaluator`** (`crates/brain-services/src/query/evaluators/search.rs`) executing query tokenization, token deduplication, posting list lookups against `snapshot.search()`, per-entity matched tokens calculation, `SearchMetadata` score computation (`matched_tokens.len() as f64`), metadata enrichment, temporal validity filtering, confidence thresholding, ordering, and pagination slicing.

**Architecture:** Evaluator lives in `crates/brain-services/src/query/evaluators/search.rs`. `evaluate` tokenizes `query.query_text` via `SearchToken::tokenize`, deduplicates tokens, and queries `snapshot.search().search_entities(&tokens)`. For each candidate entity, matched tokens are computed by checking token posting membership. Canonical pipeline order: Candidate Retrieval $\rightarrow$ Per-Entity Match Enrichment $\rightarrow$ Temporal Filter $\rightarrow$ Confidence Filter $\rightarrow$ Deterministic Sort $\rightarrow$ Pagination.

**Tech Stack:** Rust (edition 2021), `brain-domain`, `uuid`.

## Global Constraints
- `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test search_evaluator_tests` must pass cleanly.
- `SearchEvaluator` MUST BE 100% stateless and MUST NOT mutate projection snapshots.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: Lexical Search Evaluator & Integration Tests | ✅ Completed | `06ac9ca` |
| **M1 Checkpoint** | **Lexical Posting & Match Contract Verification** | ✅ Completed | `06ac9ca` |

---

### Task 1: Lexical Search Evaluator & Integration Tests

**Files:**
- Modify: `crates/brain-services/src/query/evaluators/search.rs`
- Create: `crates/brain-services/tests/search_evaluator_tests.rs`

- [ ] **Step 1: Write failing integration test `crates/brain-services/tests/search_evaluator_tests.rs`**

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

fn setup_search_test_snapshot() -> (KnowledgeEntityId, KnowledgeEntityId, Arc<ProjectionSnapshot>) {
    let e_a = KnowledgeEntityId(Uuid::from_u128(100));
    let e_b = KnowledgeEntityId(Uuid::from_u128(200));

    let mut adj_reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp_reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats_reducer = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search_reducer = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    let now = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));

    // Entity A: "graph database engine"
    let f1 = FactVersionId(Uuid::new_v4());
    let a1 = AssertionId(Uuid::new_v4());
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
            kind: AssertionKind::Attribute,
            subject: e_a.clone(),
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Literal("graph database engine".to_string()),
        }),
    };

    // Entity B: "relational database query"
    let f2 = FactVersionId(Uuid::new_v4());
    let a2 = AssertionId(Uuid::new_v4());
    let event2 = FactEvent::FactRecorded {
        fact: FactVersion {
            id: f2,
            assertion_id: a2,
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
            id: a2,
            kind: AssertionKind::Attribute,
            subject: e_b.clone(),
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Literal("relational database query".to_string()),
        }),
    };

    for ev in &[&event1, &event2] {
        let _ = adj_reducer.apply_event(ev);
        let _ = temp_reducer.apply_event(ev);
        let _ = stats_reducer.apply_event(ev);
        let _ = search_reducer.apply_event(ev);
    }

    let snapshot = ProjectionSnapshot::new(
        Arc::new(adj_reducer.state().clone()),
        Arc::new(temp_reducer.state().clone()),
        Arc::new(stats_reducer.state().clone()),
        Arc::new(search_reducer.state().clone()),
        Watermark(2),
    );

    (e_a, e_b, Arc::new(snapshot))
}

#[test]
fn test_search_empty_and_whitespace_query() {
    let (_, _, snapshot) = setup_search_test_snapshot();

    let query_empty = LexicalSearchQuery {
        query_text: "".to_string(),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res = SearchEvaluator::evaluate(&snapshot, &query_empty).unwrap();
    assert_eq!(res.total_matched, 0);
    assert!(res.matches.is_empty());
}

#[test]
fn test_search_lexical_token_match_and_metadata() {
    let (e_a, e_b, snapshot) = setup_search_test_snapshot();

    // Query "database" matches both Entity A and Entity B
    let query_db = LexicalSearchQuery {
        query_text: "database".to_string(),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res = SearchEvaluator::evaluate(&snapshot, &query_db).unwrap();
    assert_eq!(res.total_matched, 2);
    assert_eq!(res.matches[0].entity_id, e_a);
    assert_eq!(res.matches[0].search_metadata.as_ref().unwrap().score, 1.0);
    assert_eq!(res.matches[1].entity_id, e_b);

    // Query "graph" matches only Entity A
    let query_graph = LexicalSearchQuery {
        query_text: "graph".to_string(),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res_g = SearchEvaluator::evaluate(&snapshot, &query_graph).unwrap();
    assert_eq!(res_g.total_matched, 1);
    assert_eq!(res_g.matches[0].entity_id, e_a);
    assert_eq!(res_g.matches[0].search_metadata.as_ref().unwrap().matched_tokens, vec!["graph"]);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test search_evaluator_tests
```
Expected: FAIL (unimplemented stub returning empty matches).

- [ ] **Step 3: Implement `SearchEvaluator` in `src/query/evaluators/search.rs`**

```rust
// crates/brain-services/src/query/evaluators/search.rs
use crate::query::errors::*;
use crate::query::filters::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::GraphNodeId;
use brain_domain::projection::search_index::SearchToken;
use brain_domain::EntityId;
use std::collections::HashSet;

/// Stateless evaluator for lexical search queries.
pub struct SearchEvaluator;

impl SearchEvaluator {
    /// Evaluates lexical search query against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        query: &LexicalSearchQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let mut tokens = SearchToken::tokenize(&query.query_text);
        if tokens.is_empty() {
            return Ok(QueryFacadeResult {
                matches: vec![],
                total_matched: 0,
                metadata: QueryResponseMetadata {
                    execution_duration_us: 0,
                    snapshot_watermark: snapshot.watermark().0,
                },
            });
        }

        let mut seen_tokens = HashSet::new();
        tokens.retain(|t| seen_tokens.insert(t.as_str().to_string()));

        let discovered_entities = snapshot.search().search_entities(&tokens);
        if discovered_entities.is_empty() {
            return Ok(QueryFacadeResult {
                matches: vec![],
                total_matched: 0,
                metadata: QueryResponseMetadata {
                    execution_duration_us: 0,
                    snapshot_watermark: snapshot.watermark().0,
                },
            });
        }

        let mut candidates = Vec::with_capacity(discovered_entities.len());
        for entity_id in discovered_entities {
            let node_id = GraphNodeId(EntityId(entity_id.0));
            let degree = snapshot.graph().degree(&node_id);
            let stats = snapshot.statistics().get(&entity_id);

            let active_facts_count = stats.map_or(0, |s| s.active_facts_count);
            let average_confidence = stats.map_or(
                Confidence::new(0.0).unwrap(),
                |s| Confidence::new(s.average_confidence()).unwrap_or_else(|_| Confidence::new(0.0).unwrap()),
            );

            let satisfies_temporal = match query.temporal_mode {
                TemporalMode::CurrentActive => active_facts_count > 0 || stats.is_none(),
                TemporalMode::ValidAt(at_ts) => !snapshot.temporal().facts_at(&entity_id, at_ts).is_empty(),
                TemporalMode::AllHistorical => true,
            };

            if satisfies_temporal {
                let mut matched_tokens = Vec::new();
                for token in &tokens {
                    let token_entities = snapshot.search().search_entities(&[token.clone()]);
                    if token_entities.contains(&entity_id) {
                        matched_tokens.push(token.as_str().to_string());
                    }
                }

                let score = matched_tokens.len() as f64;

                candidates.push(EntityMatch {
                    entity_id,
                    active_facts_count,
                    average_confidence,
                    graph_metadata: Some(GraphMetadata {
                        in_degree: degree.in_degree,
                        out_degree: degree.out_degree,
                    }),
                    search_metadata: Some(SearchMetadata {
                        matched_tokens,
                        score,
                    }),
                });
            }
        }

        filter_by_confidence(&mut candidates, query.confidence_filter.as_ref());

        let ordering = query.ordering.clone().unwrap_or_else(|| QueryOrdering {
            field: SortField::Confidence,
            direction: SortDirection::Descending,
        });
        sort_matches(&mut candidates, Some(&ordering));
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
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test search_evaluator_tests
```
Expected: PASS cleanly.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement SearchEvaluator lexical search and match scoring"
```
