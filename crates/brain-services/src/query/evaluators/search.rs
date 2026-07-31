//! Stateless evaluator for lexical search queries.

use crate::query::errors::*;
use crate::query::filters::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::GraphNodeId;
use brain_domain::projection::search_index::SearchToken;
use brain_domain::EntityId;
use std::collections::{HashMap, HashSet};

/// Stateless evaluator for lexical search queries.
pub struct SearchEvaluator;

impl SearchEvaluator {
    /// Constructs a standardized empty result response.
    fn empty_result(snapshot: &ProjectionSnapshot) -> QueryFacadeResult {
        QueryFacadeResult {
            matches: vec![],
            total_matched: 0,
            metadata: QueryResponseMetadata {
                execution_duration_us: 0,
                snapshot_watermark: snapshot.watermark().0,
            },
        }
    }

    /// Evaluates lexical search query against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        query: &LexicalSearchQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let mut tokens = SearchToken::tokenize(&query.query_string);
        if tokens.is_empty() {
            return Ok(Self::empty_result(snapshot));
        }

        // Deduplicate query tokens
        let mut seen_tokens = HashSet::new();
        tokens.retain(|t| seen_tokens.insert(t.0.clone()));

        // Single pass over tokens building entity_to_tokens map (O(tokens) index lookups)
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

        if entity_to_tokens.is_empty() {
            return Ok(Self::empty_result(snapshot));
        }

        // 2. Candidate Enrichment & Temporal Filtering
        let mut candidates = Vec::with_capacity(entity_to_tokens.len());
        for (entity_id, matched_terms) in entity_to_tokens {
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
                    search_metadata: Some(SearchMetadata { matched_terms }),
                });
            }
        }

        // 3. Pipeline Execution (Confidence Filter -> Ordering -> Paginate)
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
