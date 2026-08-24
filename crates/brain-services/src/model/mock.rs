//! Deterministic in-memory mock model provider for offline and contract testing.

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use brain_core::errors::BrainError;
use brain_core::model::{
    GenerationChunk, GenerationRequest, ModelDescriptor, ModelProvider, TokenUsage,
};

/// Configuration for a scripted mock response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptedResponse {
    /// Optional thinking tokens.
    #[serde(default)]
    pub thinking: Option<String>,
    /// Text tokens to emit sequentially.
    #[serde(default)]
    pub tokens: Vec<String>,
    /// Optional tool calls to emit.
    #[serde(default)]
    pub tool_calls: Vec<(String, String, serde_json::Value)>,
    /// Optional simulated error to yield.
    #[serde(default)]
    pub error: Option<String>,
    /// Finish reason (defaults to "end_turn").
    #[serde(default)]
    pub finish_reason: Option<String>,
}

impl Default for ScriptedResponse {
    fn default() -> Self {
        Self {
            thinking: None,
            tokens: vec!["Hello from mock model provider.".to_string()],
            tool_calls: Vec::new(),
            error: None,
            finish_reason: Some("end_turn".to_string()),
        }
    }
}

/// Thread-safe deterministic mock model provider.
#[derive(Debug, Clone)]
pub struct DeterministicMockProvider {
    supported_models: Vec<ModelDescriptor>,
    scripted_queue: Arc<Mutex<VecDeque<ScriptedResponse>>>,
    default_response: Arc<Mutex<Option<ScriptedResponse>>>,
    /// Monotonic source for `[brain-tool:]` sentinel call IDs.
    sentinel_counter: Arc<std::sync::atomic::AtomicUsize>,
}

impl Default for DeterministicMockProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DeterministicMockProvider {
    /// Creates a new `DeterministicMockProvider` with standard mock models.
    pub fn new() -> Self {
        let models = vec![
            ModelDescriptor {
                id: "brain-default".to_string(),
                name: "Brain Default (Mock Local)".to_string(),
                provider: "mock".to_string(),
                context_window: 128000,
                max_output_tokens: 8192,
                supports_thinking: true,
                supports_tools: true,
                is_default: true,
            },
            ModelDescriptor {
                id: "claude-3-7-sonnet-latest".to_string(),
                name: "Claude 3.7 Sonnet (Mock)".to_string(),
                provider: "mock".to_string(),
                context_window: 200000,
                max_output_tokens: 64000,
                supports_thinking: true,
                supports_tools: true,
                is_default: false,
            },
            ModelDescriptor {
                id: "claude-3-5-haiku-latest".to_string(),
                name: "Claude 3.5 Haiku (Mock)".to_string(),
                provider: "mock".to_string(),
                context_window: 200000,
                max_output_tokens: 8192,
                supports_thinking: false,
                supports_tools: true,
                is_default: false,
            },
            ModelDescriptor {
                id: "deepseek-r1:latest".to_string(),
                name: "DeepSeek R1 (Mock)".to_string(),
                provider: "mock".to_string(),
                context_window: 64000,
                max_output_tokens: 16384,
                supports_thinking: true,
                supports_tools: false,
                is_default: false,
            },
            ModelDescriptor {
                id: "qwen2.5-coder:32b".to_string(),
                name: "Qwen 2.5 Coder (Mock)".to_string(),
                provider: "mock".to_string(),
                context_window: 128000,
                max_output_tokens: 8192,
                supports_thinking: false,
                supports_tools: true,
                is_default: false,
            },
        ];

        Self {
            supported_models: models,
            scripted_queue: Arc::new(Mutex::new(scripted_queue_from_env_spec(
                std::env::var("BRAIN_MOCK_SCRIPTED_RESPONSES").ok().as_deref(),
            ))),
            default_response: Arc::new(Mutex::new(None)),
            sentinel_counter: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Creates a new `DeterministicMockProvider` with custom models.
    pub fn with_models(models: Vec<ModelDescriptor>) -> Self {
        Self {
            supported_models: models,
            scripted_queue: Arc::new(Mutex::new(scripted_queue_from_env_spec(
                std::env::var("BRAIN_MOCK_SCRIPTED_RESPONSES").ok().as_deref(),
            ))),
            default_response: Arc::new(Mutex::new(None)),
            sentinel_counter: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Enqueues a scripted response to be consumed by the next generation stream.
    pub fn enqueue_response(&self, response: ScriptedResponse) {
        self.scripted_queue.lock().push_back(response);
    }

    /// Sets a persistent fallback response when the scripted queue is empty.
    pub fn set_default_response(&self, response: ScriptedResponse) {
        *self.default_response.lock() = Some(response);
    }
}

/// Extracts a deterministic tool call from a `[brain-tool:NAME]` or
/// `[brain-tool:NAME|{json}]` sentinel embedded in the last user prompt.
fn sentinel_tool_call(
    prompt: &str,
    counter: &std::sync::atomic::AtomicUsize,
) -> Option<(String, String, serde_json::Value)> {
    let start = prompt.find("[brain-tool:")?;
    let rest = &prompt[start + "[brain-tool:".len()..];
    let end = rest.find(']')?;
    let spec = &rest[..end];
    let (name, input) = match spec.split_once('|') {
        Some((n, j)) => (
            n.trim().to_string(),
            serde_json::from_str::<serde_json::Value>(j.trim())
                .unwrap_or(serde_json::json!({})),
        ),
        None => (spec.trim().to_string(), serde_json::json!({})),
    };
    if name.is_empty() {
        return None;
    }
    let n = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    Some((format!("call_mock_{}", n), name, input))
}

/// Builds the scripted queue seed from a `BRAIN_MOCK_SCRIPTED_RESPONSES`
/// spec (JSON array of `ScriptedResponse`). Malformed specs warn once and
/// degrade to the default queue so provider behavior never regresses.
fn scripted_queue_from_env_spec(spec: Option<&str>) -> VecDeque<ScriptedResponse> {
    let Some(raw) = spec else {
        return VecDeque::new();
    };
    match serde_json::from_str::<Vec<ScriptedResponse>>(raw) {
        Ok(list) => list.into_iter().collect(),
        Err(e) => {
            tracing::warn!(%e, "ignoring malformed BRAIN_MOCK_SCRIPTED_RESPONSES");
            VecDeque::new()
        }
    }
}

#[async_trait]
impl ModelProvider for DeterministicMockProvider {
    fn provider_name(&self) -> &str {
        "mock"
    }

    fn supported_models(&self) -> Vec<ModelDescriptor> {
        self.supported_models.clone()
    }

    async fn stream_generation(
        &self,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
    ) -> Result<BoxStream<'static, Result<GenerationChunk, BrainError>>, BrainError> {
        if cancellation_token.is_cancelled() {
            return Err(BrainError::Cancelled {
                message: "Generation was cancelled before starting".to_string(),
            });
        }

        let last_user_prompt = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == brain_core::model::ChatRole::User)
            .and_then(|m| {
                m.content.iter().find_map(|c| match c {
                    brain_core::model::MessageContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
            })
            .unwrap_or_else(|| "Hello".to_string());

        let scripted = {
            let mut queue = self.scripted_queue.lock();
            queue
                .pop_front()
                .or_else(|| self.default_response.lock().clone())
        };

        // Prompt sentinel takes effect only when nothing was explicitly scripted.
        let scripted = match scripted {
            Some(s) => Some(s),
            None => sentinel_tool_call(&last_user_prompt, &self.sentinel_counter).map(
                |(id, name, input)| ScriptedResponse {
                    thinking: None,
                    tokens: vec![format!("Invoking tool {}.", name)],
                    tool_calls: vec![(id, name, input)],
                    error: None,
                    finish_reason: Some("tool_use".to_string()),
                },
            ),
        };

        let response = scripted.unwrap_or_else(|| ScriptedResponse {
            thinking: Some(
                "Analyzing user request in deterministic mock engine...".to_string(),
            ),
            tokens: vec![format!("Mock response to: {}", last_user_prompt)],
            tool_calls: Vec::new(),
            error: None,
            finish_reason: Some("end_turn".to_string()),
        });

        let mut chunks: Vec<Result<GenerationChunk, BrainError>> = Vec::new();

        // 1. Thinking block if present
        if let Some(thinking_text) = response.thinking {
            chunks.push(Ok(GenerationChunk::ThinkingStart));
            chunks.push(Ok(GenerationChunk::ThinkingDelta {
                text: thinking_text,
            }));
            chunks.push(Ok(GenerationChunk::ThinkingEnd));
        }

        // 2. Simulated tool calls
        for (tool_id, tool_name, tool_input) in response.tool_calls {
            chunks.push(Ok(GenerationChunk::ToolUse {
                id: tool_id,
                name: tool_name,
                input: tool_input,
            }));
        }

        // 3. Text tokens
        let mut total_output_tokens = 0;
        for token in response.tokens {
            total_output_tokens += token.len().max(1);
            chunks.push(Ok(GenerationChunk::TextDelta { text: token }));
        }

        // 4. Simulated Error vs Completion
        if let Some(err_msg) = response.error {
            chunks.push(Err(BrainError::Model {
                model_id: request.model.clone(),
                message: err_msg,
            }));
        } else {
            let finish_reason = response
                .finish_reason
                .unwrap_or_else(|| "end_turn".to_string());
            chunks.push(Ok(GenerationChunk::Completed {
                finish_reason,
                usage: TokenUsage {
                    input_tokens: 15,
                    output_tokens: total_output_tokens,
                },
            }));
        }

        // Wrap chunks into an asynchronous stream with simulated chunk pacing and cancellation checks
        let token_clone = cancellation_token.clone();
        let delay_ms = std::env::var("BRAIN_MOCK_CHUNK_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let chunk_stream = futures::stream::unfold(
            (chunks.into_iter(), token_clone),
            move |(mut iter, token)| async move {
                if token.is_cancelled() {
                    return None;
                }
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                } else {
                    tokio::task::yield_now().await;
                }
                if token.is_cancelled() {
                    return None;
                }
                iter.next().map(|chunk| (chunk, (iter, token)))
            },
        );

        Ok(chunk_stream.boxed())
    }
}

#[cfg(test)]
mod sentinel_tests {
    use super::*;
    use brain_core::model::{ChatRole, ModelChatMessage};
    use futures::StreamExt;

    async fn collect(provider: &DeterministicMockProvider, prompt: &str) -> Vec<GenerationChunk> {
        let request = GenerationRequest {
            model: "brain-default".to_string(),
            messages: vec![ModelChatMessage::text(ChatRole::User, prompt)],
            system_prompt: None,
            tools: Vec::new(),
            thinking_budget: None,
        };
        let stream = provider
            .stream_generation(request, CancellationToken::new())
            .await
            .unwrap();
        stream.map(|c| c.unwrap()).collect::<Vec<_>>().await
    }

    #[tokio::test]
    async fn sentinel_prompt_emits_single_tool_call_with_parsed_input() {
        let provider = DeterministicMockProvider::new();
        let chunks = collect(
            &provider,
            "please run [brain-tool:bash|{\"command\":\"ls build\"}] now",
        )
        .await;
        let mut ids = Vec::new();
        let mut names = Vec::new();
        let mut inputs = Vec::new();
        for c in &chunks {
            if let GenerationChunk::ToolUse { id, name, input } = c {
                ids.push(id.clone());
                names.push(name.clone());
                inputs.push(input.clone());
            }
        }
        assert_eq!(names, vec!["bash".to_string()]);
        assert_eq!(inputs, vec![serde_json::json!({"command": "ls build"})]);
        assert!(ids[0].starts_with("call_mock_"));
        match chunks.last().unwrap() {
            GenerationChunk::Completed { finish_reason, .. } => {
                assert_eq!(finish_reason, "tool_use");
            }
            other => panic!("expected Completed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn plain_prompt_emits_no_tool_calls() {
        let provider = DeterministicMockProvider::new();
        let chunks = collect(&provider, "just say hi").await;
        assert!(
            chunks
                .iter()
                .all(|c| !matches!(c, GenerationChunk::ToolUse { .. }))
        );
    }

    #[tokio::test]
    async fn bare_sentinel_without_json_yields_empty_input() {
        let provider = DeterministicMockProvider::new();
        let chunks = collect(&provider, "use [brain-tool:search] please").await;
        let found = chunks.iter().any(|c| matches!(
            c,
            GenerationChunk::ToolUse { name, input, .. }
                if name == "search" && *input == serde_json::json!({})
        ));
        assert!(found);
    }
}

#[cfg(test)]
mod scripted_env_tests {
    use super::*;

    #[test]
    fn valid_spec_seeds_queue_in_order() {
        let spec = r#"[
            {"tokens":["Round one text."],"tool_calls":[["call_fb_1","bash",{"command":"echo one"}]],"finish_reason":"tool_use"},
            {"tokens":["Round two wraps up."],"finish_reason":"end_turn"}
        ]"#;
        let queue = scripted_queue_from_env_spec(Some(spec));
        assert_eq!(queue.len(), 2);
        let first = queue[0].clone();
        assert_eq!(first.tool_calls.len(), 1);
        assert_eq!(first.tool_calls[0].0, "call_fb_1");
        assert_eq!(first.tool_calls[0].1, "bash");
        assert_eq!(first.finish_reason.as_deref(), Some("tool_use"));
        assert_eq!(queue[1].tokens, vec!["Round two wraps up.".to_string()]);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let queue = scripted_queue_from_env_spec(Some(r#"[{"tool_calls":[["c","bash",{}]]}]"#));
        assert_eq!(queue.len(), 1);
        assert!(queue[0].thinking.is_none());
        assert!(queue[0].tokens.is_empty());
        assert!(queue[0].error.is_none());
        assert_eq!(queue[0].finish_reason.as_deref(), None);
    }

    #[test]
    fn malformed_spec_yields_empty_queue() {
        assert!(scripted_queue_from_env_spec(Some("{not json")).is_empty());
    }

    #[test]
    fn absent_spec_yields_empty_queue() {
        assert!(scripted_queue_from_env_spec(None).is_empty());
    }
}
