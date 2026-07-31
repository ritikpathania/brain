//! Stateless evaluator for compound hybrid queries.

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
    /// Internal helper performing lexical candidate discovery.
    fn retrieve_lexical_candidates(
        snapshot: &ProjectionSnapshot,
        query_string: &str,
    ) -> HashMap<KnowledgeEntityId, Vec<String>> {
        let mut tokens = SearchToken::tokenize(query_string);
        if tokens.is_empty() {
            return HashMap::new();
        }

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

        entity_to_tokens
    }

    /// Internal helper performing graph neighborhood expansion.
    fn retrieve_neighborhood_candidates(
        snapshot: &ProjectionSnapshot,
        root_entity: &KnowledgeEntityId,
        max_hops: usize,
    ) -> Vec<KnowledgeEntityId> {
        let root_node_id = GraphNodeId(EntityId(root_entity.0));
        let degree = snapshot.graph().degree(&root_node_id);
        let stats = snapshot.statistics().get(root_entity);

        if degree.in_degree == 0 && degree.out_degree == 0 && stats.is_none() {
            return Vec::new();
        }

        let mut visited: HashSet<KnowledgeEntityId> = HashSet::new();
        let mut queue: VecDeque<(KnowledgeEntityId, usize)> = VecDeque::new();
        let mut discovered: Vec<KnowledgeEntityId> = Vec::new();

        queue.push_back((*root_entity, 0));
        visited.insert(*root_entity);

        while let Some((curr_entity, depth)) = queue.pop_front() {
            discovered.push(curr_entity);

            if depth < max_hops {
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

                neighbors.sort_by_key(|a| a.0);
                neighbors.dedup();

                for neighbor in neighbors {
                    if visited.insert(neighbor) {
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }
        }

        discovered
    }

    /// Evaluates compound hybrid search query against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        query: &HybridSearchQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let mut candidates_map: BTreeMap<KnowledgeEntityId, EntityMatch> = BTreeMap::new();

        // 1. Lexical Candidate Discovery Pass
        if !query.query_string.is_empty() {
            let lexical_map = Self::retrieve_lexical_candidates(snapshot, &query.query_string);
            for (entity_id, matched_terms) in lexical_map {
                let node_id = GraphNodeId(EntityId(entity_id.0));
                let degree = snapshot.graph().degree(&node_id);
                let stats = snapshot.statistics().get(&entity_id);

                let active_facts_count = stats.map_or(0, |s| s.active_facts_count);
                let average_confidence = stats.map_or(Confidence::new(0.0).unwrap(), |s| {
                    Confidence::new(s.average_confidence())
                        .unwrap_or_else(|_| Confidence::new(0.0).unwrap())
                });

                candidates_map.insert(
                    entity_id,
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

        // 2. Graph Neighborhood Expansion Discovery Pass & Deterministic Candidate Fusion
        if let Some(ref root_entity) = query.root_entity {
            let discovered_nodes = Self::retrieve_neighborhood_candidates(snapshot, root_entity, 1);
            for curr_entity in discovered_nodes {
                let node_id = GraphNodeId(EntityId(curr_entity.0));
                let degree = snapshot.graph().degree(&node_id);
                let stats = snapshot.statistics().get(&curr_entity);

                let active_facts_count = stats.map_or(0, |s| s.active_facts_count);
                let average_confidence = stats.map_or(Confidence::new(0.0).unwrap(), |s| {
                    Confidence::new(s.average_confidence())
                        .unwrap_or_else(|_| Confidence::new(0.0).unwrap())
                });

                // Merge metadata blocks exhaustively
                candidates_map
                    .entry(curr_entity)
                    .and_modify(|existing| {
                        if existing.graph_metadata.is_none() {
                            existing.graph_metadata = Some(GraphMetadata {
                                in_degree: degree.in_degree,
                                out_degree: degree.out_degree,
                            });
                        }
                    })
                    .or_insert_with(|| EntityMatch {
                        entity_id: curr_entity,
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

        // 3. Post-Fusion Pipeline Execution (Temporal Filter -> Confidence Filter -> Ordering -> Paginate)
        let mut candidates: Vec<EntityMatch> = candidates_map.into_values().collect();

        candidates.retain(|candidate| match query.temporal_mode {
            TemporalMode::CurrentActive => {
                candidate.active_facts_count > 0
                    || snapshot.statistics().get(&candidate.entity_id).is_none()
            }
            TemporalMode::ValidAt(at_ts) => !snapshot
                .temporal()
                .facts_at(&candidate.entity_id, at_ts)
                .is_empty(),
            TemporalMode::AllHistorical => true,
        });

        filter_by_confidence(&mut candidates, query.confidence_filter.as_ref());

        let default_ordering = QueryOrdering {
            field: SortField::Confidence,
            direction: SortDirection::Descending,
        };
        sort_matches(
            &mut candidates,
            query.ordering.as_ref().or(Some(&default_ordering)),
        );
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
