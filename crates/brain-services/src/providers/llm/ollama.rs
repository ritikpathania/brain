//! Ollama LLM provider communicating with Ollama `/api/generate`.

use std::time::Duration;

use brain_core::agents::ChatAgent;
use brain_core::errors::BrainError;
use brain_domain::SessionId;
use serde::{Deserialize, Serialize};

use crate::providers::client::{block_on_future, map_reqwest_error, validate_response_status};

/// Default timeout in seconds for Ollama requests.
const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// LLM provider communicating with an Ollama instance.
#[derive(Clone, Debug)]
pub struct OllamaLlmProvider {
    client: reqwest::Client,
    model: String,
    base_url: String,
    name: String,
    timeout: Duration,
}

#[derive(Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    #[serde(default)]
    response: String,
}

impl OllamaLlmProvider {
    /// Creates a new `OllamaLlmProvider` with target model and base URL.
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
        }
    }

    /// Sets a custom request timeout.
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

    /// Asynchronously generates a completion for the given prompt.
    pub async fn generate_async(&self, prompt: &str) -> Result<String, BrainError> {
        let endpoint = format!("{}/api/generate", self.base_url.trim_end_matches('/'));
        let payload = OllamaGenerateRequest {
            model: &self.model,
            prompt,
            stream: false,
        };

        let response = self
            .client
            .post(&endpoint)
            .json(&payload)
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, &endpoint, &self.name, self.timeout))?;

        let body = validate_response_status(response, &self.name, &self.model).await?;
        let parsed: OllamaGenerateResponse =
            serde_json::from_str(&body).map_err(|e| BrainError::Validation {
                message: format!(
                    "{} failed to parse JSON response: {}. Body: {}",
                    self.name, e, body
                ),
            })?;

        Ok(parsed.response)
    }

    /// Synchronously generates a completion for the given prompt.
    pub fn generate(&self, prompt: &str) -> Result<String, BrainError> {
        let slf = self.clone();
        let prompt_str = prompt.to_string();
        block_on_future(async move { slf.generate_async(&prompt_str).await })
    }
}

impl Default for OllamaLlmProvider {
    fn default() -> Self {
        Self::new("llama3", "http://localhost:11434")
    }
}

impl ChatAgent for OllamaLlmProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn chat(&self, _session_id: SessionId, prompt: &str) -> Result<String, BrainError> {
        self.generate(prompt)
    }
}
