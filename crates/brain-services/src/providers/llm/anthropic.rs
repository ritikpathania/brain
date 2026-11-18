//! Anthropic LLM provider communicating with Anthropic `/v1/messages`.

use std::time::Duration;

use brain_core::agents::ChatAgent;
use brain_core::errors::BrainError;
use brain_domain::SessionId;
use serde::{Deserialize, Serialize};

use crate::providers::client::{block_on_future, map_reqwest_error, validate_response_status};

/// Default timeout in seconds for Anthropic requests.
const DEFAULT_TIMEOUT_SECS: u64 = 20;

/// Default maximum tokens to generate.
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Default Anthropic API version.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// LLM provider communicating with Anthropic Messages API.
#[derive(Clone, Debug)]
pub struct AnthropicLlmProvider {
    client: reqwest::Client,
    model: String,
    api_key: Option<String>,
    base_url: String,
    name: String,
    timeout: Duration,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct AnthropicMessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<AnthropicMessage<'a>>,
    temperature: f32,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct AnthropicMessagesResponse {
    #[serde(default)]
    content: Vec<AnthropicContentBlock>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: String,
}

impl AnthropicLlmProvider {
    /// Creates a new `AnthropicLlmProvider` with target model.
    ///
    /// Reads `ANTHROPIC_API_KEY` from the environment if present.
    pub fn new(model: impl Into<String>) -> Self {
        let model = model.into();
        let name = format!("anthropic-{}", model);
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
        let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECS);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            client,
            model,
            api_key,
            base_url: "https://api.anthropic.com".to_string(),
            name,
            timeout,
            max_tokens: DEFAULT_MAX_TOKENS,
            temperature: 0.2,
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

    /// Configures request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        self
    }

    /// Configures maximum generation tokens.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Configures generation temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
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
        let api_key = match &self.api_key {
            Some(key) if !key.is_empty() => key.clone(),
            _ => {
                return Err(BrainError::Authorization {
                    message: "ANTHROPIC_API_KEY environment variable not set or empty".to_string(),
                });
            }
        };

        let endpoint = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let payload = AnthropicMessagesRequest {
            model: &self.model,
            max_tokens: self.max_tokens,
            messages: vec![AnthropicMessage {
                role: "user",
                content: prompt,
            }],
            temperature: self.temperature,
        };

        let response = self
            .client
            .post(&endpoint)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, &endpoint, &self.name, self.timeout))?;

        let body = validate_response_status(response, &self.name, &self.model).await?;
        let parsed: AnthropicMessagesResponse =
            serde_json::from_str(&body).map_err(|e| BrainError::Validation {
                message: format!(
                    "{} failed to parse JSON response: {}. Body: {}",
                    self.name, e, body
                ),
            })?;

        for block in parsed.content {
            if block.content_type == "text" {
                return Ok(block.text);
            }
        }

        Err(BrainError::Validation {
            message: format!("{} returned no text content block in response", self.name),
        })
    }

    /// Synchronously generates a completion for the given prompt.
    pub fn generate(&self, prompt: &str) -> Result<String, BrainError> {
        let slf = self.clone();
        let prompt_str = prompt.to_string();
        block_on_future(async move { slf.generate_async(&prompt_str).await })
    }
}

impl Default for AnthropicLlmProvider {
    fn default() -> Self {
        Self::new("claude-3-5-sonnet-20240620")
    }
}

impl ChatAgent for AnthropicLlmProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn chat(&self, _session_id: SessionId, prompt: &str) -> Result<String, BrainError> {
        self.generate(prompt)
    }
}
