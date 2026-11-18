//! Default routing ModelGateway implementation and native ChatAgent adapter for Brain services.

use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use futures::StreamExt;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use brain_core::agents::ChatAgent;
use brain_core::errors::BrainError;
use brain_core::model::{
    ChatRole, GenerationChunk, GenerationRequest, MessageContentBlock, ModelDescriptor,
    ModelGateway, ModelProvider, TokenUsage,
};
use brain_domain::SessionId;

/// Adapter bridging a synchronous/native `ChatAgent` (`OllamaLlmProvider`, `OpenAiLlmProvider`,
/// `AnthropicLlmProvider`) into an async streaming `ModelProvider` for `ModelGateway`.
pub struct ChatAgentModelProvider {
    agent: Arc<dyn ChatAgent>,
    provider_label: String,
    descriptors: Vec<ModelDescriptor>,
}

impl ChatAgentModelProvider {
    /// Wraps a `ChatAgent` as a `ModelProvider` advertising its primary model ID.
    pub fn new(
        agent: Arc<dyn ChatAgent>,
        provider_label: impl Into<String>,
        is_default: bool,
    ) -> Self {
        let provider_label = provider_label.into();
        let agent_name = agent.name().to_string();
        let descriptor = ModelDescriptor {
            id: agent_name.clone(),
            name: format!("{} ({})", agent_name, provider_label),
            provider: provider_label.clone(),
            context_window: 128000,
            max_output_tokens: 8192,
            supports_thinking: false,
            supports_tools: false,
            is_default,
        };
        Self {
            agent,
            provider_label,
            descriptors: vec![descriptor],
        }
    }

    /// Returns a reference to the underlying `ChatAgent`.
    pub fn agent(&self) -> &Arc<dyn ChatAgent> {
        &self.agent
    }
}

#[async_trait]
impl ModelProvider for ChatAgentModelProvider {
    fn provider_name(&self) -> &str {
        &self.provider_label
    }

    fn supported_models(&self) -> Vec<ModelDescriptor> {
        self.descriptors.clone()
    }

    async fn stream_generation(
        &self,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
    ) -> Result<BoxStream<'static, Result<GenerationChunk, BrainError>>, BrainError> {
        if cancellation_token.is_cancelled() {
            return Err(BrainError::Cancelled {
                message: "Generation cancelled before dispatch".to_string(),
            });
        }

        let mut prompt_parts = Vec::new();
        if let Some(sys) = &request.system_prompt {
            if !sys.trim().is_empty() {
                prompt_parts.push(format!("System: {}", sys.trim()));
            }
        }
        for msg in &request.messages {
            let role_label = match msg.role {
                ChatRole::System => "System",
                ChatRole::User => "User",
                ChatRole::Assistant => "Assistant",
            };
            for block in &msg.content {
                if let MessageContentBlock::Text { text } = block {
                    prompt_parts.push(format!("{}: {}", role_label, text));
                }
            }
        }
        let formatted_prompt = prompt_parts.join("\n");
        let input_tokens = formatted_prompt.split_whitespace().count().max(1);
        let agent = Arc::clone(&self.agent);

        let response_text =
            tokio::task::spawn_blocking(move || agent.chat(SessionId::new(), &formatted_prompt))
                .await
                .map_err(|e| BrainError::Internal {
                    message: format!("ChatAgent task join error: {}", e),
                })??;

        if cancellation_token.is_cancelled() {
            return Err(BrainError::Cancelled {
                message: "Generation cancelled after provider completion".to_string(),
            });
        }

        let output_tokens = response_text.split_whitespace().count().max(1);
        let chunks = vec![
            Ok(GenerationChunk::TextDelta {
                text: response_text,
            }),
            Ok(GenerationChunk::Completed {
                finish_reason: "end_turn".to_string(),
                usage: TokenUsage {
                    input_tokens,
                    output_tokens,
                    ..Default::default()
                },
            }),
        ];

        Ok(stream::iter(chunks).boxed())
    }
}

/// Production implementation of `ModelGateway`.
pub struct DefaultModelGateway {
    providers: Arc<RwLock<Vec<Arc<dyn ModelProvider>>>>,
    alias_map: Arc<RwLock<HashMap<String, String>>>,
}

impl Default for DefaultModelGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultModelGateway {
    /// Creates an empty `DefaultModelGateway` with standard aliases.
    pub fn new() -> Self {
        let mut aliases = HashMap::new();
        aliases.insert("sonnet".to_string(), "claude-3-7-sonnet-latest".to_string());
        aliases.insert("haiku".to_string(), "claude-3-5-haiku-latest".to_string());
        aliases.insert("deepseek".to_string(), "deepseek-r1:latest".to_string());
        aliases.insert("r1".to_string(), "deepseek-r1:latest".to_string());
        aliases.insert("qwen".to_string(), "qwen2.5-coder:32b".to_string());
        aliases.insert("coder".to_string(), "qwen2.5-coder:32b".to_string());
        aliases.insert("default".to_string(), "brain-default".to_string());

        Self {
            providers: Arc::new(RwLock::new(Vec::new())),
            alias_map: Arc::new(RwLock::new(aliases)),
        }
    }

    /// Creates a `DefaultModelGateway` with a deterministic mock provider pre-registered.
    pub fn with_mock_provider(mock_provider: Arc<dyn ModelProvider>) -> Self {
        let gateway = Self::new();
        gateway.register_provider(mock_provider);
        gateway
    }

    /// Registers a new `ModelProvider`.
    pub fn register_provider(&self, provider: Arc<dyn ModelProvider>) {
        self.providers.write().push(provider);
    }

    /// Registers a model alias.
    pub fn register_alias(&self, alias: impl Into<String>, target_model_id: impl Into<String>) {
        self.alias_map
            .write()
            .insert(alias.into().to_lowercase(), target_model_id.into());
    }

    fn find_provider_for_model(
        &self,
        model_id: &str,
    ) -> Option<(Arc<dyn ModelProvider>, ModelDescriptor)> {
        let providers = self.providers.read();
        for p in providers.iter() {
            for m in p.supported_models() {
                if m.id == model_id {
                    return Some((Arc::clone(p), m));
                }
            }
        }
        None
    }
}

#[async_trait]
impl ModelGateway for DefaultModelGateway {
    fn list_models(&self) -> Vec<ModelDescriptor> {
        let providers = self.providers.read();
        let mut result = Vec::new();
        for p in providers.iter() {
            result.extend(p.supported_models());
        }
        result
    }

    fn resolve_model(&self, query: Option<&str>) -> Result<ModelDescriptor, BrainError> {
        let all_models = self.list_models();
        if all_models.is_empty() {
            return Err(BrainError::Model {
                model_id: query.unwrap_or("none").to_string(),
                message: "No model providers registered in ModelGateway".to_string(),
            });
        }

        let q = match query {
            Some(s) if !s.trim().is_empty() => s.trim().to_lowercase(),
            _ => {
                // Find default model or first available
                if let Some(def) = all_models.iter().find(|m| m.is_default) {
                    return Ok(def.clone());
                }
                return Ok(all_models[0].clone());
            }
        };

        // 1. Direct ID match
        if let Some(m) = all_models.iter().find(|m| m.id.to_lowercase() == q) {
            return Ok(m.clone());
        }

        // 2. Alias match
        let target_id = {
            let aliases = self.alias_map.read();
            aliases.get(&q).cloned()
        };

        if let Some(target) = target_id {
            if let Some(m) = all_models.iter().find(|m| m.id == target) {
                return Ok(m.clone());
            }
        }

        // 3. Substring match
        if let Some(m) = all_models
            .iter()
            .find(|m| m.id.to_lowercase().contains(&q) || m.name.to_lowercase().contains(&q))
        {
            return Ok(m.clone());
        }

        // 4. Return explicit error if explicit query did not match any registered model or alias
        Err(BrainError::Model {
            model_id: q.clone(),
            message: format!("Unknown model '{}'", q),
        })
    }

    async fn stream_generation(
        &self,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
    ) -> Result<BoxStream<'static, Result<GenerationChunk, BrainError>>, BrainError> {
        if cancellation_token.is_cancelled() {
            return Err(BrainError::Cancelled {
                message: "Generation stream cancelled before dispatch".to_string(),
            });
        }

        let resolved = self.resolve_model(Some(&request.model))?;
        let provider = self
            .find_provider_for_model(&resolved.id)
            .map(|(p, _)| p)
            .or_else(|| {
                let providers = self.providers.read();
                providers.first().cloned()
            })
            .ok_or_else(|| BrainError::Model {
                model_id: request.model.clone(),
                message: format!("No provider registered for model '{}'", request.model),
            })?;

        provider
            .stream_generation(request, cancellation_token)
            .await
    }
}
