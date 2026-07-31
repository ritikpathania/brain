# Phase 5.3.2 — Neighborhood Evaluator Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-services` (`src/query/evaluators/neighborhood.rs`)

---

## 1. Executive Summary & Semantic Contract

Phase 5.3.2 implements the **`NeighborhoodEvaluator`** (`brain-services::query::evaluators::neighborhood`). The evaluator executes node neighborhood graph expansion from a `root_entity` up to `max_hops` depth against `snapshot.graph()`, enriches candidates with graph degree and entity statistics, and passes candidates through our canonical query processing pipeline.

### Core Architectural Invariants:
1. **Deterministic BFS Traversal**: Graph expansion uses a Queue-based BFS up to `max_hops`. Discovered outgoing and incoming neighbors are deduplicated (`neighbors.sort(); neighbors.dedup();`) before queuing to guarantee 100% deterministic traversal independent of hash iteration.
2. **Strict Hop Semantics**:
   - `max_hops = 0`: Returns only `root_entity` (if it exists).
   - `max_hops = N`: Explores neighbors up to $N$ hops.
3. **Cycle Safety & Duplicate Prevention**: Maintains a `visited: HashSet<KnowledgeEntityId>` to guarantee that cyclic graphs (`A -> B -> A`), bidirectional edges (`A <-> B`), or diamond graphs (`A -> B -> D`, `A -> C -> D`) do not produce duplicate candidates or infinite loops.
4. **Idempotent Missing Root Handling**: If `root_entity` does not exist in the graph, the evaluator returns `Ok(QueryFacadeResult)` with `matches = []` and `total_matched = 0`.
5. **Deterministic Fallback Invariant**: If entity statistics are absent, `active_facts_count` defaults to `0` and `average_confidence` defaults to `Confidence::new(0.0).unwrap()`.
6. **Canonical Pipeline Execution**:
   ```text
   BFS Candidate Discovery ──► Metadata Enrichment ──► Temporal Filter ──► Confidence Filter ──► Deterministic Sort ──► Pagination
   ```

---

## 2. Evaluation Algorithm & Flow

```rust
use crate::query::errors::*;
use crate::query::filters::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::GraphNodeId;
use std::collections::{HashSet, VecDeque};

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
                
                // Collect outgoing and incoming neighbors
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

                // Deduplicate and sort for deterministic queue expansion
                neighbors.sort_by(|a, b| a.0.cmp(&b.0));
                neighbors.dedup();

                for neighbor in neighbors {
                    if visited.insert(neighbor.clone()) {
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }
        }

        // 2. Candidate Enrichment
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

            // Apply TemporalMode filter during candidate enrichment
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

        // 3. Pipeline Execution (Confidence Filter -> Explicit Sort -> Paginate)
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

---

## 3. Verification & Testing Strategy

1. **Unit & Contract Tests (`crates/brain-services/tests/neighborhood_evaluator_tests.rs`)**:
   - `test_neighborhood_max_hops_zero`: Verifies `max_hops = 0` returns only `root_entity`.
   - `test_neighborhood_bidirectional_edge`: Verifies bidirectional edge `A <-> B` visits B exactly once.
   - `test_neighborhood_diamond_graph_traversal`: Verifies diamond graph `A -> B -> D`, `A -> C -> D` visits D exactly once in deterministic order.
   - `test_neighborhood_missing_root_entity`: Verifies query on missing entity returns empty matches without error.
   - `test_neighborhood_temporal_and_confidence_filter`: Verifies candidate temporal filtering and confidence thresholding.
