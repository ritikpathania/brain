//! OpenAI LLM provider communicating with OpenAI `/v1/chat/completions`.

use std::time::Duration;

use brain_core::agents::ChatAgent;
use brain_core::errors::BrainError;
use brain_domain::SessionId;
use serde::{Deserialize, Serialize};

use crate::providers::client::{block_on_future, map_reqwest_error, validate_response_status};

/// Default timeout in seconds for OpenAI requests.
const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// LLM provider communicating with OpenAI's Chat Completions API.
#[derive(Clone, Debug)]
pub struct OpenAiLlmProvider {
    client: reqwest::Client,
    model: String,
    api_key: Option<String>,
    base_url: String,
    name: String,
    timeout: Duration,
    temperature: f32,
}

#[derive(Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAiMessage<'a>>,
    temperature: f32,
}

#[derive(Serialize)]
struct OpenAiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    #[serde(default)]
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Deserialize)]
struct OpenAiResponseMessage {
    #[serde(default)]
    content: String,
}

impl OpenAiLlmProvider {
    /// Creates a new `OpenAiLlmProvider` with target model.
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
                    message: "OPENAI_API_KEY environment variable not set or empty".to_string(),
                });
            }
        };

        let endpoint = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let payload = OpenAiChatRequest {
            model: &self.model,
            messages: vec![OpenAiMessage {
                role: "user",
                content: prompt,
            }],
            temperature: self.temperature,
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
        let parsed: OpenAiChatResponse =
            serde_json::from_str(&body).map_err(|e| BrainError::Validation {
                message: format!(
                    "{} failed to parse JSON response: {}. Body: {}",
                    self.name, e, body
                ),
            })?;

        if let Some(first_choice) = parsed.choices.into_iter().next() {
            Ok(first_choice.message.content)
        } else {
            Err(BrainError::Validation {
                message: format!("{} returned empty choices array in response", self.name),
            })
        }
    }

    /// Synchronously generates a completion for the given prompt.
    pub fn generate(&self, prompt: &str) -> Result<String, BrainError> {
        let slf = self.clone();
        let prompt_str = prompt.to_string();
        block_on_future(async move { slf.generate_async(&prompt_str).await })
    }
}

impl Default for OpenAiLlmProvider {
    fn default() -> Self {
        Self::new("gpt-4o-mini")
    }
}

impl ChatAgent for OpenAiLlmProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn chat(&self, _session_id: SessionId, prompt: &str) -> Result<String, BrainError> {
        self.generate(prompt)
    }
}
