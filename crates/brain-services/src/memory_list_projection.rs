use brain_core::projection::{ProjectionContext, ProjectionQuery, Projector};
use brain_domain::{Edge, Node};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Query parameters for filtering MemoryListProjection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryListQuery {
    /// Limit on the number of items returned.
    pub limit: usize,
}
impl ProjectionQuery for MemoryListQuery {}

/// Concrete view containing a sorted collection of memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryListProjection {
    /// Ordered list of active graph nodes.
    pub items: Vec<Node>,
}

/// Projector implementation mapping canonical graph structures to MemoryListProjection.
pub struct MemoryListProjector;

impl Projector<MemoryListProjection, MemoryListQuery> for MemoryListProjector {
    fn project(&self, context: &ProjectionContext<MemoryListQuery>) -> MemoryListProjection {
        let mut nodes: Vec<Node> = context.graph.nodes.values().cloned().collect();
        // Sort deterministically to satisfy the Projection Determinism invariant
        nodes.sort_by_key(|a| a.id.to_string());

        let limit = context.query.limit;
        if nodes.len() > limit {
            nodes.truncate(limit);
        }

        MemoryListProjection { items: nodes }
    }
}

/// Query parameters for runtime search projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchProjectionQuery {
    /// The search query term.
    pub query: String,
    /// The maximum number of results to return.
    pub limit: usize,
}
impl ProjectionQuery for SearchProjectionQuery {}

/// Results returned by runtime search projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchProjectionResult {
    /// The scored nodes matching the search query.
    pub items: Vec<(Node, i64)>,
    /// The connected edges connected to the matched nodes.
    pub edges: Vec<Edge>,
}

/// A native runtime projector implementing search/retrieval query semantics.
pub struct SearchProjector;

/// Tokenize and normalize text by removing punctuation, lowercasing, and skipping stop-words.
fn tokenize(text: &str) -> HashSet<String> {
    let stop_words: HashSet<&str> = [
        "a", "an", "the", "and", "or", "but", "is", "are", "was", "were", "to", "of", "in", "on",
        "at", "for", "with", "by", "about", "as", "this", "that", "these", "those", "it", "its",
        "you", "your", "my", "up", "down", "out", "off",
    ]
    .iter()
    .cloned()
    .collect();

    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty() && s.len() > 1 && !stop_words.contains(s))
        .map(|s| s.to_string())
        .collect()
}

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

        // Tokenization for exact match weights
        let query_tokens = tokenize(&context.query.query);
        let matcher = SkimMatcherV2::default();

        // 1. Exact token overlap lookups and fuzzy matching
        for node in context.graph.nodes.values() {
            let mut base_score = 0;
            let label_tokens = tokenize(&node.label);

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
