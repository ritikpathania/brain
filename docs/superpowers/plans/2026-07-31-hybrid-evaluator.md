# Phase 5.3.4 — Hybrid Evaluator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 5.3.4 **`HybridEvaluator`** (`crates/brain-services/src/query/evaluators/hybrid.rs`) composing multi-modal lexical search and graph neighborhood expansion into a deterministic `BTreeMap` candidate fusion pipeline with exhaustive field merge rules, post-fusion temporal filtering, confidence thresholding, ordering, and pagination slicing.

**Architecture:** Evaluator lives in `crates/brain-services/src/query/evaluators/hybrid.rs`. `evaluate` executes lexical candidate retrieval (if `query.query_text` is present) and graph neighborhood candidate retrieval (if `query.root_entity` is present), fusing candidates into a `BTreeMap<KnowledgeEntityId, EntityMatch>` to guarantee 100% deterministic merge ordering regardless of pass execution order. Metadata blocks (`search_metadata` and `graph_metadata`) are merged cleanly when an entity is discovered by both retrieval paths. Canonical pipeline order: Multi-Modal Discovery $\rightarrow$ Candidate Fusion $\rightarrow$ Post-Fusion Temporal Filter $\rightarrow$ Confidence Filter $\rightarrow$ Deterministic Sort $\rightarrow$ Pagination.

**Tech Stack:** Rust (edition 2021), `brain-domain`, `uuid`.

## Global Constraints
- `DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test hybrid_evaluator_tests` must pass cleanly.
- `HybridEvaluator` MUST BE 100% stateless and MUST NOT mutate projection snapshots.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: Hybrid Evaluator & Integration Tests | ⬜ Pending | |
| **M1 Checkpoint** | **Multi-Modal Fusion & Contract Verification** | ⬜ Pending | |

---

### Task 1: Hybrid Evaluator & Integration Tests

**Files:**
- Modify: `crates/brain-services/src/query/evaluators/hybrid.rs`
- Create: `crates/brain-services/tests/hybrid_evaluator_tests.rs`

- [ ] **Step 1: Write failing integration test `crates/brain-services/tests/hybrid_evaluator_tests.rs`**

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

fn setup_hybrid_test_snapshot() -> (KnowledgeEntityId, KnowledgeEntityId, KnowledgeEntityId, Arc<ProjectionSnapshot>) {
    let e_a = KnowledgeEntityId(Uuid::from_u128(100));
    let e_b = KnowledgeEntityId(Uuid::from_u128(200));
    let e_c = KnowledgeEntityId(Uuid::from_u128(300));

    let mut adj_reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp_reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats_reducer = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search_reducer = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    let now = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));

    // Entity A: "graph database" (conf 0.95), connected to Entity B
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
            kind: AssertionKind::Relationship,
            subject: e_a.clone(),
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Entity(e_b.clone()),
        }),
    };

    // Entity A also has search literal "graph database"
    let f2 = FactVersionId(Uuid::new_v4());
    let a2 = AssertionId(Uuid::new_v4());
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
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Value(LiteralValue::String("graph database".to_string())),
        }),
    };

    // Entity C: "relational database"
    let f3 = FactVersionId(Uuid::new_v4());
    let a3 = AssertionId(Uuid::new_v4());
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
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Value(LiteralValue::String("relational database".to_string())),
        }),
    };

    for ev in &[&event1, &event2, &event3] {
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
        Watermark(3),
    );

    (e_a, e_b, e_c, Arc::new(snapshot))
}

#[test]
fn test_hybrid_multi_modal_fusion_and_metadata_merge() {
    let (e_a, e_b, e_c, snapshot) = setup_hybrid_test_snapshot();

    // Query combines search_text "graph" AND root_entity e_a with max_hops 1
    let query_hybrid = HybridSearchQuery {
        query_text: Some("graph".to_string()),
        root_entity: Some(e_a.clone()),
        max_hops: Some(1),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res = HybridEvaluator::evaluate(&snapshot, &query_hybrid).unwrap();
    // Candidates: e_a (search + graph root), e_b (graph neighbor of e_a)
    assert_eq!(res.total_matched, 2);

    // Entity A was found by BOTH lexical search and graph expansion -> metadata merged!
    let match_a = res.matches.iter().find(|m| m.entity_id == e_a).unwrap();
    assert!(match_a.search_metadata.is_some());
    assert!(match_a.graph_metadata.is_some());
    assert_eq!(match_a.search_metadata.as_ref().unwrap().matched_terms, vec!["graph"]);

    // Entity B was found by graph expansion only -> graph_metadata present, search_metadata None
    let match_b = res.matches.iter().find(|m| m.entity_id == e_b).unwrap();
    assert!(match_b.search_metadata.is_none());
    assert!(match_b.graph_metadata.is_some());
}

#[test]
fn test_hybrid_lexical_only_and_neighborhood_only() {
    let (e_a, e_b, e_c, snapshot) = setup_hybrid_test_snapshot();

    // Lexical only
    let query_lexical = HybridSearchQuery {
        query_text: Some("relational".to_string()),
        root_entity: None,
        max_hops: None,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res_lex = HybridEvaluator::evaluate(&snapshot, &query_lexical).unwrap();
    assert_eq!(res_lex.total_matched, 1);
    assert_eq!(res_lex.matches[0].entity_id, e_c);

    // Neighborhood only
    let query_neigh = HybridSearchQuery {
        query_text: None,
        root_entity: Some(e_a.clone()),
        max_hops: Some(1),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res_neigh = HybridEvaluator::evaluate(&snapshot, &query_neigh).unwrap();
    assert_eq!(res_neigh.total_matched, 2);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test hybrid_evaluator_tests
```
Expected: FAIL (unimplemented stub returning empty matches).

- [ ] **Step 3: Implement `HybridEvaluator` in `src/query/evaluators/hybrid.rs`**

```rust
// crates/brain-services/src/query/evaluators/hybrid.rs
use crate::query::errors::*;
use crate::query::filters::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::GraphNodeId;
use brain_domain::projection::search_index::SearchToken;
use brain_domain::EntityId;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

/// Stateless evaluator for compound hybrid queries.
pub struct HybridEvaluator;

impl HybridEvaluator {
    /// Evaluates compound hybrid search query against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        query: &HybridSearchQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let mut candidates_map: BTreeMap<KnowledgeEntityId, EntityMatch> = BTreeMap::new();

        // 1. Lexical Retrieval Candidate Pass
        if let Some(ref text) = query.query_text {
            let mut tokens = SearchToken::tokenize(text);
            if !tokens.is_empty() {
                let mut seen = HashSet::new();
                tokens.retain(|t| seen.insert(t.0.clone()));

                let mut entity_to_tokens: HashMap<KnowledgeEntityId, Vec<String>> = HashMap::new();
                for token in &tokens {
                    let matches = snapshot.search().search_entities(&token.0);
                    for entity_id in matches {
                        entity_to_tokens
                            .entry(entity_id)
                            .or_default()
                            .push(token.0.clone());
                    }
                }

                for (entity_id, matched_terms) in entity_to_tokens {
                    let node_id = GraphNodeId(EntityId(entity_id.0));
                    let degree = snapshot.graph().degree(&node_id);
                    let stats = snapshot.statistics().get(&entity_id);

                    let active_facts_count = stats.map_or(0, |s| s.active_facts_count);
                    let average_confidence = stats.map_or(
                        Confidence::new(0.0).unwrap(),
                        |s| Confidence::new(s.average_confidence()).unwrap_or_else(|_| Confidence::new(0.0).unwrap()),
                    );

                    candidates_map.insert(
                        entity_id.clone(),
                        EntityMatch {
                            entity_id,
                            active_facts_count,
                            average_confidence,
                            graph_metadata: Some(GraphMetadata {
                                in_degree: degree.in_degree,
                                out_degree: degree.out_degree,
                            }),
                            search_metadata: Some(SearchMetadata { matched_terms }),
                        },
                    );
                }
            }
        }

        // 2. Graph Neighborhood Candidate Pass & Fusion
        if let Some(ref root_entity) = query.root_entity {
            let max_hops = query.max_hops.unwrap_or(1);
            let root_node_id = GraphNodeId(EntityId(root_entity.0));
            let root_degree = snapshot.graph().degree(&root_node_id);
            let root_stats = snapshot.statistics().get(root_entity);

            if root_degree.in_degree > 0 || root_degree.out_degree > 0 || root_stats.is_some() {
                let mut visited: HashSet<KnowledgeEntityId> = HashSet::new();
                let mut queue: VecDeque<(KnowledgeEntityId, usize)> = VecDeque::new();

                queue.push_back((root_entity.clone(), 0));
                visited.insert(root_entity.clone());

                while let Some((curr_entity, depth)) = queue.pop_front() {
                    let node_id = GraphNodeId(EntityId(curr_entity.0));
                    let degree = snapshot.graph().degree(&node_id);
                    let stats = snapshot.statistics().get(&curr_entity);

                    let active_facts_count = stats.map_or(0, |s| s.active_facts_count);
                    let average_confidence = stats.map_or(
                        Confidence::new(0.0).unwrap(),
                        |s| Confidence::new(s.average_confidence()).unwrap_or_else(|_| Confidence::new(0.0).unwrap()),
                    );

                    candidates_map
                        .entry(curr_entity.clone())
                        .and_modify(|existing| {
                            if existing.graph_metadata.is_none() {
                                existing.graph_metadata = Some(GraphMetadata {
                                    in_degree: degree.in_degree,
                                    out_degree: degree.out_degree,
                                });
                            }
                        })
                        .or_insert_with(|| EntityMatch {
                            entity_id: curr_entity.clone(),
                            active_facts_count,
                            average_confidence,
                            graph_metadata: Some(GraphMetadata {
                                in_degree: degree.in_degree,
                                out_degree: degree.out_degree,
                            }),
                            search_metadata: None,
                        });

                    if depth < max_hops {
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
            }
        }

        // 3. Post-Fusion Pipeline Execution (Temporal Filter -> Confidence Filter -> Ordering -> Paginate)
        let mut candidates: Vec<EntityMatch> = candidates_map.into_values().collect();

        candidates.retain(|candidate| match query.temporal_mode {
            TemporalMode::CurrentActive => candidate.active_facts_count > 0 || snapshot.statistics().get(&candidate.entity_id).is_none(),
            TemporalMode::ValidAt(at_ts) => !snapshot.temporal().facts_at(&candidate.entity_id, at_ts).is_empty(),
            TemporalMode::AllHistorical => true,
        });

        filter_by_confidence(&mut candidates, query.confidence_filter.as_ref());

        let default_ordering = QueryOrdering {
            field: SortField::Confidence,
            direction: SortDirection::Descending,
        };
        sort_matches(&mut candidates, query.ordering.as_ref().or(Some(&default_ordering)));
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
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test hybrid_evaluator_tests
```
Expected: PASS cleanly.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement HybridEvaluator multi-modal candidate fusion and metadata merging"
```
