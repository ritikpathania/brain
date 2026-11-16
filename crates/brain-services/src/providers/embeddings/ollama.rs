//! Ollama embedding provider connecting to Ollama `/api/embed` or `/api/embeddings`.

use std::time::Duration;

use brain_core::agents::EmbeddingAgent;
use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::providers::client::{block_on_future, map_reqwest_error, validate_response_status};

/// Default timeout in seconds for Ollama requests.
const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// Default embedding vector dimension for nomic-embed-text.
const DEFAULT_DIMENSION: usize = 384;

/// Embedding provider connecting to Ollama `/api/embed` with fallback to `/api/embeddings`.
#[derive(Clone, Debug)]
pub struct OllamaEmbeddingProvider {
    client: reqwest::Client,
    model: String,
    base_url: String,
    name: String,
    timeout: Duration,
    dimension: usize,
}

#[derive(Serialize)]
struct OllamaEmbedApiRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Deserialize)]
struct OllamaEmbedApiResponse {
    #[serde(default)]
    embeddings: Vec<Vec<f32>>,
}

#[derive(Serialize)]
struct OllamaLegacyEmbedRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct OllamaLegacyEmbedResponse {
    #[serde(default)]
    embedding: Vec<f32>,
}

impl OllamaEmbeddingProvider {
    /// Creates a new `OllamaEmbeddingProvider` with target model and base URL.
    pub fn new(model: impl Into<String>, base_url: impl Into<String>) -> Self {
        let model = model.into();
        let base_url = base_url.into();
        let name = format!("ollama-{}", model);
        let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECS);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            client,
            model,
            base_url,
            name,
            timeout,
            dimension: DEFAULT_DIMENSION,
        }
    }

    /// Sets the expected embedding vector dimension.
    pub fn with_dimension(mut self, dimension: usize) -> Self {
        self.dimension = dimension;
        self
    }

    /// Sets request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        self
    }

    /// Sets a custom reqwest client.
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// Returns the configured model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Returns the configured base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Asynchronously generates an embedding vector for the given text.
    pub async fn embed_async(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let primary_endpoint = format!("{}/api/embed", self.base_url.trim_end_matches('/'));
        let primary_payload = OllamaEmbedApiRequest {
            model: &self.model,
            input: text,
        };

        let response = self
            .client
            .post(&primary_endpoint)
            .json(&primary_payload)
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, &primary_endpoint, &self.name, self.timeout))?;

        let vector = if response.status() == StatusCode::NOT_FOUND {
            // Attempt fallback to legacy /api/embeddings
            let fallback_endpoint =
                format!("{}/api/embeddings", self.base_url.trim_end_matches('/'));
            let fallback_payload = OllamaLegacyEmbedRequest {
                model: &self.model,
                prompt: text,
            };

            let fallback_response = self
                .client
                .post(&fallback_endpoint)
                .json(&fallback_payload)
                .send()
                .await
                .map_err(|e| map_reqwest_error(e, &fallback_endpoint, &self.name, self.timeout))?;

            let body = validate_response_status(fallback_response, &self.name, &self.model).await?;
            let parsed: OllamaLegacyEmbedResponse =
                serde_json::from_str(&body).map_err(|e| BrainError::Validation {
                    message: format!(
                        "{} failed to parse legacy JSON response: {}. Body: {}",
                        self.name, e, body
                    ),
                })?;

            parsed.embedding
        } else {
            let body = validate_response_status(response, &self.name, &self.model).await?;
            let parsed: OllamaEmbedApiResponse =
                serde_json::from_str(&body).map_err(|e| BrainError::Validation {
                    message: format!(
                        "{} failed to parse JSON response: {}. Body: {}",
                        self.name, e, body
                    ),
                })?;

            if let Some(first_vec) = parsed.embeddings.into_iter().next() {
                first_vec
            } else {
                return Err(BrainError::Validation {
                    message: format!("{} returned empty embeddings array in response", self.name),
                });
            }
        };

        if vector.is_empty() {
            return Err(BrainError::Validation {
                message: format!("{} returned empty embedding vector", self.name),
            });
        }

        if vector.len() != self.dimension {
            return Err(BrainError::Validation {
                message: format!(
                    "{} returned embedding with dimension {}, expected {}",
                    self.name,
                    vector.len(),
                    self.dimension
                ),
            });
        }

        Ok(vector)
    }

    /// Synchronously generates an embedding vector for the given text.
    pub fn embed_vector(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let slf = self.clone();
        let text_str = text.to_string();
        block_on_future(async move { slf.embed_async(&text_str).await })
    }
}

impl Default for OllamaEmbeddingProvider {
    fn default() -> Self {
        Self::new("nomic-embed-text", "http://localhost:11434")
    }
}

impl EmbeddingProvider for OllamaEmbeddingProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        self.embed_vector(text)
    }
}

impl EmbeddingAgent for OllamaEmbeddingProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        self.embed_vector(text)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
