use crate::identifiers::NodeId;
use crate::retrieval::models::{
    Evidence, ExpansionPolicy, RetrievalExecutionContext, RetrievedCandidate, StoppingCriterion,
};
use std::collections::{HashSet, VecDeque};

/// Common trait for all retrieval candidate search engines.
pub trait RetrievalSource {
    /// Distinct identifier of the retrieval source.
    fn source_id(&self) -> &'static str;
    /// Formulates and returns candidate findings against the graph context.
    fn retrieve(&self, context: &RetrievalExecutionContext) -> Vec<RetrievedCandidate>;
}

/// Semantic vector search simulator using substring matches as base relevance.
pub struct VectorSource {
    /// String to search for.
    pub query: String,
}

impl VectorSource {
    /// Creates a new `VectorSource`.
    pub fn new(query: String) -> Self {
        Self { query }
    }
}

impl RetrievalSource for VectorSource {
    fn source_id(&self) -> &'static str {
        "vector"
    }

    fn retrieve(&self, context: &RetrievalExecutionContext) -> Vec<RetrievedCandidate> {
        let mut candidates = Vec::new();
        let graph = context.graph;
        for (&id, node) in &graph.nodes {
            if node
                .label
                .to_lowercase()
                .contains(&self.query.to_lowercase())
            {
                candidates.push(RetrievedCandidate {
                    node_id: id,
                    source_id: self.source_id(),
                    local_score: 0.85,
                    explanation_fragments: vec![Evidence::SemanticMatch { similarity: 0.85 }],
                });
            }
        }
        candidates
    }
}

/// Inverted-index text search simulator.
pub struct KeywordSource {
    /// Word prefix to match.
    pub query: String,
}

impl KeywordSource {
    /// Creates a new `KeywordSource`.
    pub fn new(query: String) -> Self {
        Self { query }
    }
}

impl RetrievalSource for KeywordSource {
    fn source_id(&self) -> &'static str {
        "keyword"
    }

    fn retrieve(&self, context: &RetrievalExecutionContext) -> Vec<RetrievedCandidate> {
        let mut candidates = Vec::new();
        let graph = context.graph;
        for (&id, node) in &graph.nodes {
            if node
                .label
                .to_lowercase()
                .contains(&self.query.to_lowercase())
            {
                candidates.push(RetrievedCandidate {
                    node_id: id,
                    source_id: self.source_id(),
                    local_score: 0.75,
                    explanation_fragments: vec![Evidence::KeywordHit { occurrences: 1 }],
                });
            }
        }
        candidates
    }
}

/// Relational BFS path expander.
pub struct GraphExpansionSource {
    /// Starter seed node ids.
    pub seeds: Vec<NodeId>,
    /// Traversal constraint policy settings.
    pub policy: ExpansionPolicy,
}

impl GraphExpansionSource {
    /// Creates a new `GraphExpansionSource`.
    pub fn new(seeds: Vec<NodeId>, policy: ExpansionPolicy) -> Self {
        Self { seeds, policy }
    }
}

impl RetrievalSource for GraphExpansionSource {
    fn source_id(&self) -> &'static str {
        "graph_expansion"
    }

    fn retrieve(&self, context: &RetrievalExecutionContext) -> Vec<RetrievedCandidate> {
        let mut candidates = Vec::new();
        if self.seeds.is_empty() {
            return candidates;
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        let mut max_depth = usize::MAX;
        let mut max_visited = usize::MAX;
        let mut min_confidence = 0.0;

        for criterion in &self.policy.criteria {
            match *criterion {
                StoppingCriterion::MaxDepth(d) => max_depth = d,
                StoppingCriterion::MaxVisitedNodes(v) => max_visited = v,
                StoppingCriterion::MinConfidence(c) => min_confidence = c,
            }
        }

        let adjacency = context.analytics.adjacency();
        let graph = context.graph;

        for &seed in &self.seeds {
            if graph.nodes.contains_key(&seed) {
                queue.push_back((seed, 0));
                visited.insert(seed);
            }
        }

        while let Some((u, depth)) = queue.pop_front() {
            if visited.len() > max_visited {
                break;
            }

            if !self.seeds.contains(&u) {
                let score = if depth > 0 { 0.5 / (depth as f64) } else { 0.5 };
                candidates.push(RetrievedCandidate {
                    node_id: u,
                    source_id: self.source_id(),
                    local_score: score,
                    explanation_fragments: vec![Evidence::GraphTraversal {
                        depth,
                        from: self.seeds[0],
                    }],
                });
            }

            if depth >= max_depth {
                continue;
            }

            for &neighbor in adjacency.neighbors(u) {
                if visited.contains(&neighbor) {
                    continue;
                }

                if let Some(edge) = graph
                    .edges
                    .values()
                    .find(|e| e.source == u && e.target == neighbor)
                {
                    if (edge.provenance.confidence as f64) < min_confidence {
                        continue;
                    }
                    if let Some(ref filter) = self.policy.relation_filter {
                        if !filter.contains(&edge.relation) {
                            continue;
                        }
                    }
                }

                visited.insert(neighbor);
                queue.push_back((neighbor, depth + 1));
            }
        }

        candidates
    }
}
