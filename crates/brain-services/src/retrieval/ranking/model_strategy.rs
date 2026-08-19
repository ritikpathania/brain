use crate::retrieval::ranking::feature_provider::{
    FeatureContext, FeatureExtractor, FeatureProvider, RankingDecay,
};
use crate::retrieval::ranking::score_ranker::ScoreRanker;
use brain_core::errors::BrainError;
use brain_core::retrieval::{EmbeddingLookup, RankingStrategy, RetrievalRequest};
use brain_domain::{Node, NodeId};
use brain_storage::connection::SqliteConnectionManager;
use brain_storage::r2d2::Pool;
use std::collections::HashMap;
use std::sync::Arc;

/// Runtime strategy that uses machine learned rankers.
/// Combines dynamic similarity score generation and database metadata context
/// to evaluate candidate relevance in a leak-free and train/serve skew-free pipeline.
pub struct ModelRankingStrategy {
    ranker: Arc<dyn ScoreRanker>,
    provider: Arc<dyn FeatureProvider>,
    query_embedding_service: Arc<dyn brain_core::retrieval::QueryEmbeddingService>,
    embedding_lookup: Arc<dyn EmbeddingLookup>,
    pool: Pool<SqliteConnectionManager>,
    reference_time: u64,
    decay: RankingDecay,
}

impl ModelRankingStrategy {
    /// Instantiates a new ModelRankingStrategy.
    pub fn new(
        ranker: Arc<dyn ScoreRanker>,
        provider: Arc<dyn FeatureProvider>,
        query_embedding_service: Arc<dyn brain_core::retrieval::QueryEmbeddingService>,
        embedding_lookup: Arc<dyn EmbeddingLookup>,
        pool: Pool<SqliteConnectionManager>,
        reference_time: u64,
        decay: RankingDecay,
    ) -> Self {
        Self {
            ranker,
            provider,
            query_embedding_service,
            embedding_lookup,
            pool,
            reference_time,
            decay,
        }
    }
}

impl RankingStrategy for ModelRankingStrategy {
    fn rank(&self, request: &RetrievalRequest, nodes: Vec<Node>) -> Result<Vec<Node>, BrainError> {
        let start_time = std::time::Instant::now();
        if nodes.is_empty() {
            return Ok(nodes);
        }

        // 1. Load context features for all candidate NodeIds
        let node_ids: Vec<NodeId> = nodes.iter().map(|n| n.id).collect();
        let contexts = self.provider.load_contexts(&node_ids)?;

        // 2. Query FTS (BM25) matching scores dynamically to match FtsRetriever offline output
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!(
                "Failed to acquire connection for model ranking strategy: {}",
                e
            ),
            source: Some(Box::new(e)),
        })?;

        let sanitized = sanitize_fts_query(&request.query);
        let mut fts_scores = HashMap::new();
        if !sanitized.is_empty() {
            let mut stmt = conn
                .prepare(
                    "SELECT n.id, bm25(node_search) \
                     FROM nodes n \
                     JOIN node_search ns ON n.rowid = ns.rowid \
                     WHERE node_search MATCH ?1",
                )
                .map_err(|e| BrainError::Storage {
                    message: format!("Failed to prepare MATCH query: {}", e),
                    source: Some(Box::new(e)),
                })?;

            let rows = stmt
                .query_map([&sanitized], |row| {
                    let uuid_str: String = row.get(0)?;
                    let bm25_score: f64 = row.get(1)?;
                    Ok((uuid_str, bm25_score))
                })
                .map_err(|e| BrainError::Storage {
                    message: format!("Failed to query MATCH: {}", e),
                    source: Some(Box::new(e)),
                })?;

            for (uuid_str, score) in rows.flatten() {
                if let Ok(uuid) = uuid::Uuid::parse_str(&uuid_str) {
                    // Negate it to match FtsRetriever behavior (higher score = better match)
                    fts_scores.insert(NodeId(uuid), -score);
                }
            }
        }

        // 3. Compute vector semantic similarity scores dynamically
        let query_vector = self.query_embedding_service.embed_query(&request.query)?;
        let mut semantic_scores = HashMap::new();
        for node in &nodes {
            if let Some(vector) = self.embedding_lookup.lookup(&node.id)? {
                if query_vector.len() == vector.len() {
                    let score = cosine_similarity(&query_vector, &vector);
                    semantic_scores.insert(node.id, score as f64);
                }
            }
        }

        // 4. Extract features, score, and compute fingerprints to enforce train/serve skew bounds
        let extractor = FeatureExtractor::new(self.reference_time, self.decay);
        let mut scored_nodes = Vec::with_capacity(nodes.len());

        for node in nodes {
            let default_ctx = FeatureContext {
                updated_at: None,
                importance: None,
                pinned: false,
                provenance_confidence: None,
                graph_degree: None,
                access_count: None,
                last_observed_at: None,
            };
            let context = contexts.get(&node.id).unwrap_or(&default_ctx);

            let lexical = fts_scores.get(&node.id).copied();
            let semantic = semantic_scores.get(&node.id).copied();

            let features = extractor.extract(lexical, semantic, context);
            let score = self.ranker.score(&features);

            // Log fingerprints in target target="brain::telemetry::retrieval" for parity verification
            tracing::debug!(
                target: "brain::telemetry::retrieval",
                node_id = %node.id.0,
                fingerprint = %features.fingerprint(),
                score = score,
                "Computed model ranker features"
            );

            scored_nodes.push((node, score));
        }

        // 5. Sort candidate nodes descending by score with node UUID ASC fallback for absolute determinism
        scored_nodes.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.id.0.cmp(&b.0.id.0))
        });

        let duration = start_time.elapsed();
        tracing::info!(
            target: "brain::telemetry::retrieval",
            stage = "ModelRanking",
            model_name = self.ranker.name(),
            duration_ms = duration.as_millis(),
            "Model ranking stage completed"
        );

        Ok(scored_nodes.into_iter().map(|(n, _)| n).collect())
    }
}

fn sanitize_fts_query(query: &str) -> String {
    let terms: Vec<String> = query.split_whitespace().map(|s| s.to_string()).collect();

    terms.join(" OR ")
}

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
