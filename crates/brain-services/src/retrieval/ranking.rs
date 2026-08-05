/// Feature provider and extractor contracts for contextual ranking.
pub mod feature_provider;
/// Runtime ranking strategy using machine learned scoring models.
pub mod model_strategy;
/// Post-fusion candidate reranking infrastructure.
pub mod reranker;
/// Scoring facade for machine learned models.
pub mod score_ranker;

pub use model_strategy::ModelRankingStrategy;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{EmbeddingLookup, RankingStrategy, RetrievalRequest};
use brain_domain::Node;

// Temporary implementation.
// Future tokenizer/stemmer belongs in a dedicated text analysis module.
fn tokenize(text: &str) -> Vec<String> {
    use std::sync::OnceLock;
    static STOP_WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    let stop_words = STOP_WORDS.get_or_init(|| {
        [
            "a", "an", "the", "and", "or", "but", "is", "are", "was", "were", "to", "of", "in",
            "on", "at", "for", "with", "by", "about", "as", "this", "that", "these", "those", "it",
            "its", "you", "your", "my", "up", "down", "out", "off",
        ]
        .iter()
        .copied()
        .collect()
    });

    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty() && s.len() > 1 && !stop_words.contains(s))
        .map(|s| s.to_string())
        .collect()
}

/// Calculates cosine similarity between two float vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut row: Vec<usize> = (0..=len2).collect();
    for (i, c1) in s1.chars().enumerate() {
        let mut prev = i + 1;
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            let val = std::cmp::min(row[j + 1] + 1, std::cmp::min(prev + 1, row[j] + cost));
            row[j] = prev;
            prev = val;
        }
        row[len2] = prev;
    }
    row[len2]
}

fn word_similarity(q: &str, word: &str) -> f32 {
    let q_lower = q.to_lowercase();
    let w_lower = word.to_lowercase();
    if q_lower == w_lower {
        return 1.0;
    }
    if w_lower.contains(&q_lower) {
        return q_lower.len() as f32 / w_lower.len() as f32;
    }
    let dist = levenshtein_distance(&q_lower, &w_lower);
    let max_len = std::cmp::max(q_lower.len(), w_lower.len());
    if max_len > 0 {
        let sim = 1.0 - (dist as f32 / max_len as f32);
        if sim >= 0.7 {
            return sim;
        }
    }
    0.0
}

/// Lexical ranking strategy based on the BM25 TF-IDF algorithm.
#[derive(Debug, Clone)]
pub struct Bm25Ranking {
    k1: f32,
    b: f32,
}

impl Default for Bm25Ranking {
    fn default() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }
}

impl Bm25Ranking {
    /// Creates a new Bm25Ranking strategy with custom hyperparameters.
    pub fn new(k1: f32, b: f32) -> Self {
        Self { k1, b }
    }
}

impl RankingStrategy for Bm25Ranking {
    fn rank(&self, request: &RetrievalRequest, nodes: Vec<Node>) -> Result<Vec<Node>, BrainError> {
        if nodes.is_empty() {
            return Ok(nodes);
        }

        let query_tokens = tokenize(&request.query);
        if query_tokens.is_empty() {
            let mut result = nodes;
            result.sort_by_key(|n| n.id.0);
            return Ok(result);
        }

        let n = nodes.len() as f32;

        // 1. Tokenize corpus and compute document lengths
        let mut doc_tokens = Vec::with_capacity(nodes.len());
        let mut doc_lengths = Vec::with_capacity(nodes.len());
        let mut total_length = 0.0;

        for node in &nodes {
            let tokens = tokenize(&node.label);
            let len = tokens.len() as f32;
            total_length += len;

            let mut tf = HashMap::new();
            for token in tokens {
                *tf.entry(token).or_insert(0.0) += 1.0;
            }
            doc_tokens.push(tf);
            doc_lengths.push(len);
        }

        let avgdl = if n > 0.0 { total_length / n } else { 1.0 };
        let avgdl = if avgdl > 0.0 { avgdl } else { 1.0 };

        // 2. Compute document frequency and IDF for each query term (with fuzzy support)
        let mut idf_map = HashMap::new();
        for token in &query_tokens {
            let mut count = 0.0;
            for tf_map in &doc_tokens {
                let mut matches = false;
                for doc_tok in tf_map.keys() {
                    if word_similarity(token, doc_tok) > 0.0 {
                        matches = true;
                        break;
                    }
                }
                if matches {
                    count += 1.0;
                }
            }
            let idf = ((n - count + 0.5) / (count + 0.5) + 1.0).ln();
            idf_map.insert(token.clone(), idf);
        }

        // 3. Score each document
        let mut scored_nodes = Vec::with_capacity(nodes.len());
        for (idx, node) in nodes.into_iter().enumerate() {
            let mut score = 0.0;
            let doc_len = doc_lengths[idx];
            let tf_map = &doc_tokens[idx];

            for token in &query_tokens {
                let mut best_tf_val = 0.0;
                for (doc_tok, &doc_tf) in tf_map {
                    let sim = word_similarity(token, doc_tok);
                    if sim > 0.0 {
                        let tf_val = doc_tf * sim;
                        if tf_val > best_tf_val {
                            best_tf_val = tf_val;
                        }
                    }
                }
                if best_tf_val == 0.0 {
                    continue;
                }
                let idf = *idf_map.get(token).unwrap_or(&0.0);

                let numerator = best_tf_val * (self.k1 + 1.0);
                let denominator =
                    best_tf_val + self.k1 * (1.0 - self.b + self.b * (doc_len / avgdl));

                score += idf * (numerator / denominator);
            }

            // Phrase match boosting
            let label_lower = node.label.to_lowercase();
            let query_lower = request.query.to_lowercase();
            if label_lower == query_lower {
                score += 150.0;
            } else if label_lower.contains(&query_lower) || query_lower.contains(&label_lower) {
                score += 80.0;
            }

            scored_nodes.push((node, score));
        }

        // Sort by score DESC, with node ID ASC fallback for ties
        scored_nodes.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.id.0.cmp(&b.0.id.0))
        });

        Ok(scored_nodes.into_iter().map(|(n, _)| n).collect())
    }
}

/// Semantic ranking strategy based on vector cosine similarity.
pub struct EmbeddingRanking {
    query_embedding_service: Arc<dyn brain_core::retrieval::QueryEmbeddingService>,
    embedding_lookup: Arc<dyn EmbeddingLookup>,
}

impl EmbeddingRanking {
    /// Creates a new EmbeddingRanking strategy.
    pub fn new(
        query_embedding_service: Arc<dyn brain_core::retrieval::QueryEmbeddingService>,
        embedding_lookup: Arc<dyn EmbeddingLookup>,
    ) -> Self {
        Self {
            query_embedding_service,
            embedding_lookup,
        }
    }
}

impl RankingStrategy for EmbeddingRanking {
    fn rank(&self, request: &RetrievalRequest, nodes: Vec<Node>) -> Result<Vec<Node>, BrainError> {
        if nodes.is_empty() {
            return Ok(nodes);
        }

        let query_vector = self.query_embedding_service.embed_query(&request.query)?;
        let mut scored_nodes = Vec::with_capacity(nodes.len());

        for node in nodes {
            let similarity = if let Some(vector) = self.embedding_lookup.lookup(&node.id)? {
                if query_vector.len() == vector.len() {
                    cosine_similarity(&query_vector, &vector)
                } else {
                    0.0
                }
            } else {
                0.0
            };
            scored_nodes.push((node, similarity));
        }

        // Sort by similarity DESC, with node ID ASC fallback
        scored_nodes.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.id.0.cmp(&b.0.id.0))
        });

        Ok(scored_nodes.into_iter().map(|(n, _)| n).collect())
    }
}

/// Private helper that encapsulates graph traversal and weight accumulation.
struct GraphScorer<'a> {
    repos: &'a dyn RepositorySet,
}

impl<'a> GraphScorer<'a> {
    fn new(repos: &'a dyn RepositorySet) -> Self {
        Self { repos }
    }

    /// Computes the graph centrality score for a given node.
    fn score_node(&self, node_id: &brain_domain::NodeId) -> Result<f64, BrainError> {
        let connections = self.repos.edges().get_connections(node_id)?;
        Ok(connections.iter().map(|e| e.weight).sum())
    }
}

/// Graph connectivity ranking strategy based on edge weight centrality.
pub struct GraphRanking {
    repos: Arc<dyn RepositorySet>,
}

impl GraphRanking {
    /// Creates a new GraphRanking strategy.
    pub fn new(repos: Arc<dyn RepositorySet>) -> Self {
        Self { repos }
    }
}

impl RankingStrategy for GraphRanking {
    fn rank(&self, _request: &RetrievalRequest, nodes: Vec<Node>) -> Result<Vec<Node>, BrainError> {
        if nodes.is_empty() {
            return Ok(nodes);
        }

        let scorer = GraphScorer::new(self.repos.as_ref());
        let mut scored_nodes = Vec::with_capacity(nodes.len());
        for node in nodes {
            let score = scorer.score_node(&node.id)?;
            scored_nodes.push((node, score));
        }

        // Sort by graph score DESC, with node ID ASC fallback
        scored_nodes.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.id.0.cmp(&b.0.id.0))
        });

        Ok(scored_nodes.into_iter().map(|(n, _)| n).collect())
    }
}

/// An internal representation of a candidate node, its 1-based rank, and its raw score in a specific strategy.
#[derive(Debug, Clone)]
pub struct RankedCandidate {
    /// The candidate node.
    pub node: Node,
    /// The 1-based rank.
    pub rank: usize,
    /// The raw score.
    pub score: f64,
}

/// A collection of ranked candidates produced by a specific ranking pathway.
#[derive(Debug, Clone)]
pub struct RankedCandidates {
    /// The list of ranked candidates.
    pub items: Vec<RankedCandidate>,
}

impl RankedCandidates {
    /// Creates a RankedCandidates collection by ranking the nodes using a strategy.
    pub fn from_strategy(
        strategy: &dyn RankingStrategy,
        request: &RetrievalRequest,
        nodes: Vec<Node>,
    ) -> Result<Self, BrainError> {
        let ranked_nodes = strategy.rank(request, nodes)?;
        let items = ranked_nodes
            .into_iter()
            .enumerate()
            .map(|(idx, node)| RankedCandidate {
                node,
                rank: idx + 1,
                score: 0.0,
            })
            .collect();
        Ok(Self { items })
    }
}

/// Reciprocal Rank Fusion (RRF) ranking strategy combining multiple sub-strategies.
pub struct RrfRanking {
    strategies: Vec<(Arc<dyn RankingStrategy>, f64)>,
    k: f64,
}

impl RrfRanking {
    /// Creates a new RrfRanking strategy combining the given strategies and weights.
    pub fn new(strategies: Vec<(Arc<dyn RankingStrategy>, f64)>, k: f64) -> Self {
        Self { strategies, k }
    }
}

impl RankingStrategy for RrfRanking {
    fn rank(&self, request: &RetrievalRequest, nodes: Vec<Node>) -> Result<Vec<Node>, BrainError> {
        let start = std::time::Instant::now();
        if nodes.is_empty() || self.strategies.is_empty() {
            let duration = start.elapsed();
            tracing::info!(
                target: "brain::telemetry::retrieval",
                stage = "RRF",
                duration_ms = duration.as_millis(),
                input_candidate_count = nodes.len(),
                "Retrieval stage completed: RRF"
            );
            return Ok(nodes);
        }

        let mut rrf_scores = HashMap::new();

        for (strategy, weight) in &self.strategies {
            let ranked_candidates =
                RankedCandidates::from_strategy(strategy.as_ref(), request, nodes.clone())?;
            for item in ranked_candidates.items {
                let rank = item.rank as f64;
                let score = weight / (self.k + rank);
                *rrf_scores.entry(item.node.id).or_insert(0.0) += score;
            }
        }

        let mut result = nodes;
        result.sort_by(|a, b| {
            let score_a = rrf_scores.get(&a.id).cloned().unwrap_or(0.0);
            let score_b = rrf_scores.get(&b.id).cloned().unwrap_or(0.0);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });

        let duration = start.elapsed();
        tracing::info!(
            target: "brain::telemetry::retrieval",
            stage = "RRF",
            duration_ms = duration.as_millis(),
            input_candidate_count = result.len(),
            "Retrieval stage completed: RRF"
        );

        Ok(result)
    }
}
