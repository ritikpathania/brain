//! Stateless evaluator for node neighborhood graph traversal.

use crate::query::errors::*;
use crate::query::filters::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::GraphNodeId;
use brain_domain::EntityId;
use std::collections::{HashSet, VecDeque};

/// Stateless evaluator for node neighborhood graph traversal.
pub struct NeighborhoodEvaluator;

impl NeighborhoodEvaluator {
    /// Collects deduplicated, sorted adjacent neighbor entity IDs for a graph node.
    fn collect_neighbors(
        snapshot: &ProjectionSnapshot,
        node_id: &GraphNodeId,
    ) -> Vec<KnowledgeEntityId> {
        let mut neighbors = Vec::new();
        for edge_id in snapshot.graph().neighbors_out(node_id) {
            if let Some(edge) = snapshot.graph().edge(edge_id) {
                neighbors.push(KnowledgeEntityId(edge.target.0 .0));
            }
        }
        for edge_id in snapshot.graph().neighbors_in(node_id) {
            if let Some(edge) = snapshot.graph().edge(edge_id) {
                neighbors.push(KnowledgeEntityId(edge.source.0 .0));
            }
        }

        // Sort and deduplicate for deterministic traversal ordering
        neighbors.sort_by_key(|a| a.0);
        neighbors.dedup();
        neighbors
    }

    /// Evaluates node neighborhood graph expansion against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        query: &NeighborhoodQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let root_node_id = GraphNodeId(EntityId(query.root_entity.0));

        // If root entity does not exist in graph or statistics, return empty result
        let root_degree = snapshot.graph().degree(&root_node_id);
        let root_stats = snapshot.statistics().get(&query.root_entity);
        if root_degree.in_degree == 0 && root_degree.out_degree == 0 && root_stats.is_none() {
            return Ok(QueryFacadeResult {
                matches: vec![],
                total_matched: 0,
                metadata: QueryResponseMetadata {
                    execution_duration_us: 0,
                    snapshot_watermark: snapshot.watermark().0,
                },
            });
        }

        // 1. Candidate Discovery (BFS)
        let mut visited: HashSet<KnowledgeEntityId> = HashSet::new();
        let mut queue: VecDeque<(KnowledgeEntityId, usize)> = VecDeque::new();
        let mut discovered: Vec<KnowledgeEntityId> = Vec::new();

        queue.push_back((query.root_entity, 0));
        visited.insert(query.root_entity);

        while let Some((curr_entity, depth)) = queue.pop_front() {
            discovered.push(curr_entity);

            if depth < query.max_hops {
                let node_id = GraphNodeId(EntityId(curr_entity.0));
                let neighbors = Self::collect_neighbors(snapshot, &node_id);

                for neighbor in neighbors {
                    if visited.insert(neighbor) {
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }
        }

        // 2. Candidate Enrichment & Temporal Filtering
        let mut candidates = Vec::with_capacity(discovered.len());
        for entity_id in discovered {
            let node_id = GraphNodeId(EntityId(entity_id.0));
            let degree = snapshot.graph().degree(&node_id);
            let stats = snapshot.statistics().get(&entity_id);

            let active_facts_count = stats.map_or(0, |s| s.active_facts_count);
            let average_confidence = stats.map_or(Confidence::new(0.0).unwrap(), |s| {
                Confidence::new(s.average_confidence())
                    .unwrap_or_else(|_| Confidence::new(0.0).unwrap())
            });

            let satisfies_temporal = match query.temporal_mode {
                TemporalMode::CurrentActive => active_facts_count > 0 || stats.is_none(),
                TemporalMode::ValidAt(at_ts) => {
                    !snapshot.temporal().facts_at(&entity_id, at_ts).is_empty()
                }
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

        // 3. Pipeline Execution (Confidence Filter -> Explicit Default Sort -> Paginate)
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
