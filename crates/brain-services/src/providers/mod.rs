//! Native LLM and embedding providers for Ollama, OpenAI, and Anthropic.

pub(crate) mod client;
/// Semantic text embedding provider modules.
pub mod embeddings;
/// LLM chat agent provider modules.
pub mod llm;

pub use embeddings::{
    OllamaEmbeddingProvider, OpenAiCompatibleEmbeddingProvider, OpenAiEmbeddingProvider,
};
pub use llm::{AnthropicLlmProvider, OllamaLlmProvider, OpenAiLlmProvider};
