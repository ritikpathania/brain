//! OpenAI-compatible embedding provider connecting to any OpenAI-compatible `/embeddings` endpoint.

use std::time::Duration;

use brain_core::agents::EmbeddingAgent;
use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use serde::{Deserialize, Serialize};

use crate::providers::client::{block_on_future, map_reqwest_error, validate_response_status};

/// Default timeout in seconds for OpenAI-compatible embedding requests.
const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// Default embedding vector dimension for custom models.
const DEFAULT_DIMENSION: usize = 384;

/// Embedding provider connecting to any OpenAI-compatible `/embeddings` endpoint.
#[derive(Clone, Debug)]
pub struct OpenAiCompatibleEmbeddingProvider {
    client: reqwest::Client,
    model: String,
    base_url: String,
    api_key: Option<String>,
    name: String,
    timeout: Duration,
    dimension: usize,
}

#[derive(Serialize)]
struct OpenAiCompatibleRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Deserialize)]
struct OpenAiCompatibleResponse {
    #[serde(default)]
    data: Vec<OpenAiCompatibleData>,
}

#[derive(Deserialize)]
struct OpenAiCompatibleData {
    #[serde(default)]
    embedding: Vec<f32>,
}

impl OpenAiCompatibleEmbeddingProvider {
    /// Creates a new `OpenAiCompatibleEmbeddingProvider` with target model and base URL.
    ///
    /// Reads `CUSTOM_EMBEDDING_API_KEY` from the environment if present.
    pub fn new(model: impl Into<String>, base_url: impl Into<String>) -> Self {
        let model = model.into();
        let base_url = base_url.into();
        let name = format!("custom-{}", model);
        let api_key = std::env::var("CUSTOM_EMBEDDING_API_KEY").ok();
        let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECS);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            client,
            model,
            base_url,
            api_key,
            name,
            timeout,
            dimension: DEFAULT_DIMENSION,
        }
    }

    /// Sets optional API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Sets expected embedding dimension.
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

    /// Sets custom reqwest client.
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
        let endpoint = format!("{}/embeddings", self.base_url.trim_end_matches('/'));
        let payload = OpenAiCompatibleRequest {
            model: &self.model,
            input: text,
        };

        let mut req = self
            .client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .json(&payload);

        if let Some(key) = &self.api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }

        let response = req
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, &endpoint, &self.name, self.timeout))?;

        let body = validate_response_status(response, &self.name, &self.model).await?;
        let parsed: OpenAiCompatibleResponse =
            serde_json::from_str(&body).map_err(|e| BrainError::Validation {
                message: format!(
                    "{} failed to parse JSON response: {}. Body: {}",
                    self.name, e, body
                ),
            })?;

        let vector = if let Some(first_entry) = parsed.data.into_iter().next() {
            first_entry.embedding
        } else {
            return Err(BrainError::Validation {
                message: format!("{} returned empty data array in response", self.name),
            });
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

impl Default for OpenAiCompatibleEmbeddingProvider {
    fn default() -> Self {
        Self::new("custom-model", "http://localhost:8000/v1")
    }
}

impl EmbeddingProvider for OpenAiCompatibleEmbeddingProvider {
    fn name(&self) -> &'static str {
        "openai-compatible"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        self.embed_vector(text)
    }
}

impl EmbeddingAgent for OpenAiCompatibleEmbeddingProvider {
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
