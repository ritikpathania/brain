use crate::plugins::{EmbeddingProvider, RetrievalAlgorithm, StorageBackend};
use crate::stm::{STMIndex, TempNode};

pub struct EmbeddingsRetrieval;

impl EmbeddingsRetrieval {
    /// Retrieve semantically similar node IDs and their similarity scores from LTM storage
    pub fn retrieve_ltm_semantic(
        query: &str,
        provider: &dyn EmbeddingProvider,
        storage: &dyn StorageBackend,
        limit: usize,
    ) -> Result<Vec<(String, f32)>, String> {
        let query_embedding = provider.embed(query)?;
        storage.query_nearest_neighbors(&query_embedding, limit)
    }
}

impl RetrievalAlgorithm for EmbeddingsRetrieval {
    fn name(&self) -> &str {
        "embeddings"
    }

    fn retrieve(
        &self,
        _query: &str,
        _index: &STMIndex,
        _window: &[TempNode],
    ) -> Result<Vec<(TempNode, i64)>, String> {
        // STM-only semantic search is empty by default; STM uses BM25/Fuzzy, while LTM uses Hybrid
        Ok(Vec::new())
    }
}
