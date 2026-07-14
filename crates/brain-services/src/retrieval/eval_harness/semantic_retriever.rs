use crate::retrieval::eval_harness::{RetrievalChannel, RetrievalResult, Retriever};
use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::EmbeddingProvider;
use brain_storage::connection::SqliteConnectionManager;
use brain_storage::store::SqliteStorage;
use r2d2::Pool;
use std::collections::HashMap;
use std::sync::Arc;

/// A retriever implementation that queries long-term memory embeddings.
pub struct SemanticRetriever {
    pool: Pool<SqliteConnectionManager>,
    provider: Arc<dyn EmbeddingProvider>,
}

impl SemanticRetriever {
    /// Creates a new `SemanticRetriever` using the given database connection pool and embedding provider.
    pub fn new(pool: Pool<SqliteConnectionManager>, provider: Arc<dyn EmbeddingProvider>) -> Self {
        Self { pool, provider }
    }
}

impl Retriever for SemanticRetriever {
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        // 1. Generate embedding for query
        let query_vector = self.provider.embed(query)?;

        // 2. Fetch all node embeddings from database
        let storage = SqliteStorage::from_pool(self.pool.clone());
        let all_embeddings = storage.embeddings().list_all_embeddings()?;

        // 3. Compute cosine similarity for each embedding
        let mut results = Vec::new();
        for emb in all_embeddings {
            let score = cosine_similarity(&query_vector, &emb.vector);
            results.push(RetrievalResult {
                node_id: emb.node_id,
                channel_scores: HashMap::from([(RetrievalChannel::Semantic, score as f64)]),
                ranking_score: None,
            });
        }

        Ok(results)
    }

    fn normalize_query(&self, query: &str) -> Option<String> {
        Some(query.to_string())
    }

    fn executed_query(&self, _query: &str) -> Option<String> {
        Some(format!("EmbeddingSearch(model={})", self.provider.name()))
    }
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
