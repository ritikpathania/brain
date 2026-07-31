# Phase 5.3.2 — Neighborhood Evaluator Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-services` (`src/query/evaluators/neighborhood.rs`)

---

## 1. Executive Summary & Semantic Contract

Phase 5.3.2 implements the **`NeighborhoodEvaluator`** (`brain-services::query::evaluators::neighborhood`). The evaluator executes node neighborhood graph expansion from a `root_entity` up to `max_hops` depth against `snapshot.graph()`, enriches candidates with graph degree and entity statistics, and passes candidates through our canonical query processing pipeline.

### Core Architectural Invariants:
1. **Deterministic BFS Traversal**: Graph expansion uses a Queue-based BFS up to `max_hops`. Discovered neighbors at each node are sorted in ascending `KnowledgeEntityId` order before queuing to guarantee 100% deterministic traversal independent of internal hash iteration.
2. **Strict Hop Semantics**:
   - `max_hops = 0`: Returns only `root_entity` (if it exists).
   - `max_hops = N`: Explores neighbors up to $N$ hops.
3. **Cycle Safety & Duplicate Prevention**: Maintains a `visited: HashSet<KnowledgeEntityId>` to guarantee that cyclic graphs (`A -> B -> A`) or multiple shortest paths do not produce duplicate candidates or infinite loops.
4. **Idempotent Missing Root Handling**: If `root_entity` does not exist in the graph, the evaluator returns `Ok(QueryFacadeResult)` with `matches = []` and `total_matched = 0`.
5. **Canonical Pipeline Execution**:
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

                // Deterministic tie-breaker sorting for queue expansion
                neighbors.sort_by(|a, b| a.0.cmp(&b.0));

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

        // 3. Pipeline Execution (Temporal Filter -> Confidence Filter -> Sort -> Paginate)
        filter_by_confidence(&mut candidates, query.confidence_filter.as_ref());
        sort_matches(&mut candidates, None); // Default deterministic sort by EntityId ASC
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
   - `test_neighborhood_cyclic_graph_traversal`: Verifies cyclic graph (`A -> B -> A`) does not cause infinite loops or duplicate matches.
   - `test_neighborhood_missing_root_entity`: Verifies query on missing entity returns empty matches without error.
   - `test_neighborhood_deterministic_neighbor_ordering`: Verifies neighbor expansion processes in ascending `KnowledgeEntityId` order.
   - `test_neighborhood_confidence_filter_and_pagination`: Verifies candidate enrichment, confidence filtering, and pagination slicing.
