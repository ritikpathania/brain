# Phase 5.3.4 — Hybrid Evaluator Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-services` (`src/query/evaluators/hybrid.rs`)

---

## 1. Executive Summary & Semantic Contract

Phase 5.3.4 implements the **`HybridEvaluator`** (`brain-services::query::evaluators::hybrid`). The evaluator composes multi-modal retrieval by combining lexical candidate search (`LexicalSearchQuery`) and graph neighborhood expansion (`NeighborhoodQuery`), merging candidates and metadata deterministically, and running candidates through our canonical query processing pipeline.

### Core Architectural Invariants:
1. **Composition Over Duplication**: `HybridEvaluator` delegates candidate discovery to modular retrieval helpers (`retrieve_lexical_candidates` and `retrieve_neighborhood_candidates`) without duplicating index search or BFS graph traversal code.
2. **Deterministic Candidate Fusion**: Discovered candidates from lexical and graph sources are merged into a deterministic `BTreeMap<KnowledgeEntityId, EntityMatch>` keyed by entity ID.
3. **Exhaustive Field Merge Rules**:
   | EntityMatch Field | Merge Rule |
   | :--- | :--- |
   | `entity_id` | Unique key for fusion |
   | `search_metadata` | Preserved from lexical candidate pass (if present) |
   | `graph_metadata` | Preserved from graph neighborhood candidate pass (if present) |
   | `active_facts_count` | Enriched uniformly from `snapshot.statistics().get(entity_id)` |
   | `average_confidence` | Enriched uniformly from `snapshot.statistics().get(entity_id)` |
4. **Post-Fusion Temporal Filtering**: Temporal Mode filtering is executed strictly AFTER candidate fusion:
   ```text
   Multi-Modal Retrieval ──► Candidate Fusion ──► Temporal Filter ──► Confidence Filter ──► Deterministic Sort ──► Pagination
   ```
5. **Retrieval Order Independence**: Candidate fusion produces identical output regardless of the order in which lexical or graph candidate passes are executed.

---

## 2. Evaluation Algorithm & Flow

```rust
use crate::query::errors::*;
use crate::query::filters::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::GraphNodeId;
use brain_domain::projection::search_index::SearchToken;
use brain_domain::EntityId;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

pub struct HybridEvaluator;

impl HybridEvaluator {
    /// Evaluates compound hybrid search query against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        query: &HybridSearchQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let mut candidates_map: BTreeMap<KnowledgeEntityId, EntityMatch> = BTreeMap::new();

        // 1. Lexical Candidate Retrieval
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

        // 2. Graph Neighborhood Candidate Retrieval & Fusion
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

                    // Merge metadata exhaustive rules
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
            TemporalMode::CurrentActive => candidate.active_facts_count > 0,
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

---

## 3. Verification & Testing Strategy

1. **Unit & Contract Tests (`crates/brain-services/tests/hybrid_evaluator_tests.rs`)**:
   - `test_hybrid_lexical_only_retrieval`: Verifies hybrid query with only `query_text` populated.
   - `test_hybrid_neighborhood_only_retrieval`: Verifies hybrid query with only `root_entity` populated.
   - `test_hybrid_multi_modal_fusion_and_metadata_merge`: Verifies entity found by both lexical search and graph expansion has both `search_metadata` and `graph_metadata` merged cleanly without duplicates.
   - `test_hybrid_retrieval_order_independence`: Verifies fusion output is identical regardless of pass order.
   - `test_hybrid_temporal_and_confidence_filter`: Verifies post-fusion temporal mode filtering and confidence thresholding.
   - `test_hybrid_deterministic_ordering_and_pagination`: Verifies candidate sorting and pagination slicing over merged candidate sets.
