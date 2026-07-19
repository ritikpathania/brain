use brain_core::projection::{ProjectionContext, ProjectionQuery, Projector};
use brain_domain::{Edge, Node};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Query parameters for runtime search projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchProjectionQuery {
    pub query: String,
    pub limit: usize,
}
impl ProjectionQuery for SearchProjectionQuery {}

/// Results returned by runtime search projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchProjectionResult {
    pub items: Vec<(Node, i64)>,
    pub edges: Vec<Edge>,
}

/// A temporary compatibility projector implementing legacy retrieval semantics during the migration.
/// It will be replaced by native runtime retrieval after legacy removal.
pub struct SearchProjector;

impl Projector<SearchProjectionResult, SearchProjectionQuery> for SearchProjector {
    fn project(
        &self,
        context: &ProjectionContext<SearchProjectionQuery>,
    ) -> SearchProjectionResult {
        let mut scored_nodes = Vec::new();
        if context.query.query.trim().is_empty() {
            return SearchProjectionResult {
                items: Vec::new(),
                edges: Vec::new(),
            };
        }

        // Tokenization similar to legacy fuzzy.rs
        let query_tokens = crate::retrieval::fuzzy::tokenize(&context.query.query);
        let matcher = SkimMatcherV2::default();

        // 1. Exact token overlap lookups and fuzzy matching
        for node in context.graph.nodes.values() {
            let mut base_score = 0;
            let label_tokens = crate::retrieval::fuzzy::tokenize(&node.label);

            for token in &query_tokens {
                if label_tokens.contains(token) {
                    base_score += 50;
                }
            }

            let fuzzy_score = matcher
                .fuzzy_match(&node.label, &context.query.query)
                .unwrap_or(0);
            let total_score = base_score + fuzzy_score;

            if total_score > 0 {
                scored_nodes.push((node.clone(), total_score));
            }
        }

        // 2. Sort descending by score, and then alphabetically by label to keep it deterministic
        scored_nodes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.label.cmp(&b.0.label)));

        // 3. Truncate to limit
        if scored_nodes.len() > context.query.limit {
            scored_nodes.truncate(context.query.limit);
        }

        // 4. Collect connected edges from context.graph.edges in-memory for the matched nodes
        let matched_ids: HashSet<_> = scored_nodes.iter().map(|(n, _)| n.id).collect();
        let mut connected_edges = Vec::new();
        for edge in context.graph.edges.values() {
            if matched_ids.contains(&edge.source) || matched_ids.contains(&edge.target) {
                connected_edges.push(edge.clone());
            }
        }

        SearchProjectionResult {
            items: scored_nodes,
            edges: connected_edges,
        }
    }
}
