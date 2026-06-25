use crate::stm::{STMIndex, TempNode};
use crate::storage::{ExtractedEdge, ExtractedGraph, ExtractedNode};

pub trait EmbeddingProvider: Send + Sync {
    fn name(&self) -> &str;
    fn embed(&self, text: &str) -> Result<Vec<f32>, String>;
}

pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn generate(&self, prompt: &str) -> Result<String, String>;
}

pub trait RetrievalAlgorithm: Send + Sync {
    fn name(&self) -> &str;
    fn retrieve(
        &self,
        query: &str,
        index: &STMIndex,
        window: &[TempNode],
    ) -> Result<Vec<(TempNode, i64)>, String>;
}

pub trait RankingStrategy: Send + Sync {
    fn name(&self) -> &str;
    fn rank(&self, query: &str, candidates: &mut Vec<(TempNode, i64)>) -> Result<(), String>;
}

pub trait StorageBackend: Send + Sync {
    fn name(&self) -> &str;
    fn write_graph(&self, nodes: &[ExtractedNode], edges: &[ExtractedEdge]) -> Result<(), String>;
    fn query_graph(&self, query: &str) -> Result<Vec<(ExtractedNode, Vec<ExtractedEdge>)>, String>;
    fn get_updates_since(
        &self,
        timestamp: i64,
    ) -> Result<(Vec<ExtractedNode>, Vec<ExtractedEdge>, i64), String>;
    fn decay_weights(&self, half_life_secs: f64, threshold: f64) -> Result<(), String>;
    fn write_embeddings(&self, embeddings: &[(String, Vec<f32>)]) -> Result<(), String>;
    fn query_nearest_neighbors(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(String, f32)>, String>;
    fn get_connections(&self, node_ids: &[String]) -> Result<Vec<ExtractedEdge>, String>;
    fn get_nodes_by_ids(&self, ids: &[String]) -> Result<Vec<ExtractedNode>, String>;
}

pub trait MemoryExtractor: Send + Sync {
    fn name(&self) -> &str;
    fn extract(&self, stm_nodes: &[TempNode]) -> Result<ExtractedGraph, String>;
}

pub trait Exporter: Send + Sync {
    fn name(&self) -> &str;
    fn export(&self, backend: &dyn StorageBackend) -> Result<(), String>;
}

pub trait CliPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn get_subcommand_name(&self) -> &str;
    fn get_subcommand_description(&self) -> &str;
    fn handle_command(&self, args: &[String]) -> Result<(), String>;
}

// Built-in default & Noop providers
pub struct NoopEmbeddingProvider;
impl EmbeddingProvider for NoopEmbeddingProvider {
    fn name(&self) -> &str {
        "noop"
    }
    fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
        Ok(vec![0.0; 384])
    }
}

pub struct NoopLlmProvider;
impl LlmProvider for NoopLlmProvider {
    fn name(&self) -> &str {
        "noop"
    }
    fn generate(&self, prompt: &str) -> Result<String, String> {
        Ok(format!("Noop LLM response to: {}", prompt))
    }
}
