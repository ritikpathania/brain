//! OpenAI embedding provider connecting to OpenAI `/v1/embeddings`.

use std::time::Duration;

use brain_core::agents::EmbeddingAgent;
use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use serde::{Deserialize, Serialize};

use crate::providers::client::{block_on_future, map_reqwest_error, validate_response_status};

/// Default timeout in seconds for OpenAI requests.
const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// Default embedding vector dimension for text-embedding-3-small.
const DEFAULT_DIMENSION: usize = 1536;

/// Embedding provider connecting to OpenAI's Embeddings API.
#[derive(Clone, Debug)]
pub struct OpenAiEmbeddingProvider {
    client: reqwest::Client,
    model: String,
    api_key: Option<String>,
    base_url: String,
    name: String,
    timeout: Duration,
    dimension: usize,
}

#[derive(Serialize)]
struct OpenAiEmbeddingRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingResponse {
    #[serde(default)]
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingData {
    #[serde(default)]
    embedding: Vec<f32>,
}

impl OpenAiEmbeddingProvider {
    /// Creates a new `OpenAiEmbeddingProvider` with target model.
    ///
    /// Reads `OPENAI_API_KEY` from the environment if present.
    pub fn new(model: impl Into<String>) -> Self {
        let model = model.into();
        let name = format!("openai-{}", model);
        let api_key = std::env::var("OPENAI_API_KEY").ok();
        let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECS);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            client,
            model,
            api_key,
            base_url: "https://api.openai.com".to_string(),
            name,
            timeout,
            dimension: DEFAULT_DIMENSION,
        }
    }

    /// Configures an explicit API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Configures a custom base URL.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Sets the expected embedding vector dimension.
    pub fn with_dimension(mut self, dimension: usize) -> Self {
        self.dimension = dimension;
        self
    }

    /// Configures request timeout.
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
        let api_key = match &self.api_key {
            Some(key) if !key.is_empty() => key.clone(),
            _ => {
                return Err(BrainError::Authorization {
                    message: "OPENAI_API_KEY environment variable not set or empty".to_string(),
                });
            }
        };

        let endpoint = format!("{}/v1/embeddings", self.base_url.trim_end_matches('/'));
        let payload = OpenAiEmbeddingRequest {
            model: &self.model,
            input: text,
        };

        let response = self
            .client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, &endpoint, &self.name, self.timeout))?;

        let body = validate_response_status(response, &self.name, &self.model).await?;
        let parsed: OpenAiEmbeddingResponse =
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

impl Default for OpenAiEmbeddingProvider {
    fn default() -> Self {
        Self::new("text-embedding-3-small")
    }
}

impl EmbeddingProvider for OpenAiEmbeddingProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        self.embed_vector(text)
    }
}

impl EmbeddingAgent for OpenAiEmbeddingProvider {
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
