use std::collections::HashMap;
use std::sync::Arc;

use crate::config::PluginConfig;
use crate::plugins::traits::{
    CliPlugin, EmbeddingProvider, Exporter, LlmProvider, MemoryExtractor, RankingStrategy,
    RetrievalAlgorithm, StorageBackend,
};

pub struct PluginRegistry {
    pub embedding_providers: HashMap<String, Arc<dyn EmbeddingProvider>>,
    pub llm_providers: HashMap<String, Arc<dyn LlmProvider>>,
    pub retrieval_algorithms: HashMap<String, Arc<dyn RetrievalAlgorithm>>,
    pub ranking_strategies: HashMap<String, Arc<dyn RankingStrategy>>,
    pub storage_backends: HashMap<String, Arc<dyn StorageBackend>>,
    pub memory_extractors: HashMap<String, Arc<dyn MemoryExtractor>>,
    pub exporters: HashMap<String, Arc<dyn Exporter>>,
    pub cli_plugins: HashMap<String, Arc<dyn CliPlugin>>,
    pub config: PluginConfig,
}

impl PluginRegistry {
    pub fn new(config: PluginConfig) -> Self {
        Self {
            embedding_providers: HashMap::new(),
            llm_providers: HashMap::new(),
            retrieval_algorithms: HashMap::new(),
            ranking_strategies: HashMap::new(),
            storage_backends: HashMap::new(),
            memory_extractors: HashMap::new(),
            exporters: HashMap::new(),
            cli_plugins: HashMap::new(),
            config,
        }
    }

    pub fn get_embedding(&self) -> Result<Arc<dyn EmbeddingProvider>, String> {
        self.embedding_providers
            .get(&self.config.active_embedding_provider)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Active embedding provider '{}' not found",
                    self.config.active_embedding_provider
                )
            })
    }

    pub fn get_llm(&self) -> Result<Arc<dyn LlmProvider>, String> {
        self.llm_providers
            .get(&self.config.active_llm_provider)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Active LLM provider '{}' not found",
                    self.config.active_llm_provider
                )
            })
    }

    pub fn get_retrieval(&self) -> Result<Arc<dyn RetrievalAlgorithm>, String> {
        self.retrieval_algorithms
            .get(&self.config.active_retrieval_algorithm)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Active retrieval algorithm '{}' not found",
                    self.config.active_retrieval_algorithm
                )
            })
    }

    pub fn get_ranking(&self) -> Result<Arc<dyn RankingStrategy>, String> {
        self.ranking_strategies
            .get(&self.config.active_ranking_strategy)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Active ranking strategy '{}' not found",
                    self.config.active_ranking_strategy
                )
            })
    }

    pub fn get_storage(&self) -> Result<Arc<dyn StorageBackend>, String> {
        self.storage_backends
            .get(&self.config.active_storage_backend)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Active storage backend '{}' not found",
                    self.config.active_storage_backend
                )
            })
    }

    pub fn get_extractor(&self) -> Result<Arc<dyn MemoryExtractor>, String> {
        self.memory_extractors
            .get(&self.config.active_memory_extractor)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Active memory extractor '{}' not found",
                    self.config.active_memory_extractor
                )
            })
    }

    pub fn get_exporter(&self) -> Result<Arc<dyn Exporter>, String> {
        self.exporters
            .get(&self.config.active_exporter)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Active exporter '{}' not found",
                    self.config.active_exporter
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_registration_and_activation() {
        let config = PluginConfig {
            active_llm_provider: "custom-llm".to_string(),
            ..Default::default()
        };

        let mut registry = PluginRegistry::new(config);

        struct CustomLlm;
        impl LlmProvider for CustomLlm {
            fn name(&self) -> &str {
                "custom-llm"
            }
            fn generate(&self, prompt: &str) -> Result<String, String> {
                Ok(format!("custom: {}", prompt))
            }
        }

        registry
            .llm_providers
            .insert("custom-llm".to_string(), Arc::new(CustomLlm));

        let active_llm = registry.get_llm().unwrap();
        assert_eq!(active_llm.name(), "custom-llm");
        assert_eq!(active_llm.generate("hello").unwrap(), "custom: hello");
    }
}
