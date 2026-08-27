//! Default routing ModelGateway implementation for Brain services.

use async_trait::async_trait;
use futures::stream::BoxStream;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use brain_core::errors::BrainError;
use brain_core::model::{
    GenerationChunk, GenerationRequest, ModelDescriptor, ModelGateway, ModelProvider,
};

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
