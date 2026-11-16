//! Embedding provider implementations for Ollama, OpenAI, and OpenAI-compatible endpoints.

mod ollama;
mod openai;
mod openai_compatible;

pub use ollama::OllamaEmbeddingProvider;
pub use openai::OpenAiEmbeddingProvider;
pub use openai_compatible::OpenAiCompatibleEmbeddingProvider;

use brain_config::schema::{EmbeddingProvider as Backend, ModelSettings};
use brain_core::retrieval::EmbeddingProvider;
use std::sync::Arc;

/// Resolves the configured provider for the daemon's shared query embedding service.
pub fn configured_provider(settings: &ModelSettings) -> Option<Arc<dyn EmbeddingProvider>> {
    let dimension = settings.embedding_dimension();
    let ollama = || {
        Arc::new(
            OllamaEmbeddingProvider::new("nomic-embed-text", "http://localhost:11434")
                .with_dimension(dimension),
        ) as Arc<dyn EmbeddingProvider>
    };
    let openai = || {
        Arc::new(OpenAiEmbeddingProvider::new("text-embedding-3-small").with_dimension(dimension))
            as Arc<dyn EmbeddingProvider>
    };
    match settings.active_embedding_provider() {
        Backend::Ollama => Some(ollama()),
        Backend::Openai => Some(openai()),
        Backend::Custom(name) if name.eq_ignore_ascii_case("ollama") => Some(ollama()),
        Backend::Custom(name) if name.eq_ignore_ascii_case("openai") => Some(openai()),
        Backend::Custom(name)
            if name.to_ascii_lowercase().starts_with("openai-compat")
                || name.to_ascii_lowercase().starts_with("custom") =>
        {
            Some(Arc::new(
                OpenAiCompatibleEmbeddingProvider::new("custom-model", "http://localhost:8000/v1")
                    .with_dimension(dimension),
            ))
        }
        _ => None,
    }
}
