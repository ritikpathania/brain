/// Active weights snapshot provider abstractions.
pub mod active_weights;
/// Cache layers and snapshot execution management.
pub mod cache;
/// Cost calibration and feedback engine.
pub mod calibration;
/// Retrieval evaluation and benchmarking harness.
pub mod eval_harness;
/// Offline evaluation pipeline.
pub mod evaluator;
/// Experiment routing and canary deployments.
pub mod experiment;
/// Feature extraction pipeline for learned ranking.
pub mod feature_extractor;
/// Candidate fusion strategies (RRF).
pub mod fusion;
/// Graph traversal, budgeting, and analysis services.
pub mod graph_service;
/// Dynamic loader for learned ranking models.
pub mod model_loader;
/// Deserializer and dynamic factory resolution for ranking models.
pub mod model_resolver;
/// Memory retrieval orchestrator pipeline, accumulator, and builder.
pub mod pipeline;
/// Contextual ranking strategies (BM25, Embeddings, Graph, and RRF).
pub mod ranking;
/// Post-retrieval relationship expansion.
pub mod relationship_expander;
/// Concrete memory source implementations (STM, LTM, etc).
pub mod source;
/// Durable SQLite cache store backend.
pub mod sqlite_store;
/// Temporal retrieval integration, projection views, and ranking.
pub mod temporal;

use self::relationship_expander::RelationshipExpander;
use crate::mapper::to_memory_dto;
use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{CacheHydrationPolicy, RetrievalRequest};
use brain_core::services::RetrievalService;
use brain_domain::{MemoryDTO, SessionId};
use brain_session::SessionCacheManager;
use std::sync::Arc;

struct RepositoryEmbeddingLookup {
    repos: Arc<dyn RepositorySet>,
}

impl brain_core::retrieval::EmbeddingLookup for RepositoryEmbeddingLookup {
    fn lookup(&self, node_id: &brain_domain::NodeId) -> Result<Option<Vec<f32>>, BrainError> {
        self.repos
            .embeddings()
            .find_by_node_id(node_id)
            .map(|opt| opt.map(|e| e.vector))
    }
}

/// Concrete implementation of RetrievalService routing query searches across cache and storage.
pub struct RetrievalServiceImpl {
    repos: Arc<dyn RepositorySet>,
    pipeline: pipeline::RetrievalPipeline,
}

impl RetrievalServiceImpl {
    /// Creates a new RetrievalServiceImpl.
    pub fn new(
        repos: Arc<dyn RepositorySet>,
        cache_manager: Arc<SessionCacheManager>,
        registry: Arc<brain_domain::RelationRegistry>,
        query_embedding_service: Arc<dyn brain_core::retrieval::QueryEmbeddingService>,
    ) -> Self {
        let src_stm = Arc::new(source::StmMemorySource::new(
            cache_manager.clone(),
            repos.clone(),
            registry.clone(),
        ));
        let src_ltm = Arc::new(source::LtmMemorySource::new(
            repos.clone(),
            registry.clone(),
        ));
        let src_vector = Arc::new(source::SemanticMemorySource::new(
            repos.clone(),
            query_embedding_service.clone(),
        ));

        let strategy_bm25 = Arc::new(ranking::Bm25Ranking::default());
        let strategy_vector = Arc::new(ranking::EmbeddingRanking::new(
            query_embedding_service,
            Arc::new(RepositoryEmbeddingLookup {
                repos: repos.clone(),
            }),
        ));
        let rrf = Arc::new(ranking::RrfRanking::new(
            vec![(strategy_bm25, 1.0), (strategy_vector, 1.0)],
            60.0,
        ));

        let temporal_reranker = Arc::new(ranking::reranker::TemporalReranker::new());
        let pipeline = pipeline::MemoryPipelineBuilder::new()
            .register_source(src_stm)
            .register_source(src_ltm)
            .register_source(src_vector)
            .with_ranking_strategy(rrf)
            .with_policy(CacheHydrationPolicy::OnHit)
            .with_cache_manager(cache_manager)
            .with_temporal_ranking_config(brain_core::retrieval::TemporalRankingSettings {
                enabled: false,
                model: brain_core::retrieval::DecayModel::Uniform,
                half_life_seconds: 86400,
                scaling_factor: 1.0,
            })
            .register_reranker(temporal_reranker)
            .build();

        Self { repos, pipeline }
    }

    /// Creates a new RetrievalServiceImpl configured by BrainSettings.
    pub fn new_with_config(
        storage: Arc<brain_storage::SqliteStorage>,
        config: &brain_config::schema::BrainSettings,
        cache_manager: Arc<SessionCacheManager>,
        registry: Arc<brain_domain::RelationRegistry>,
        query_embedding_service: Arc<dyn brain_core::retrieval::QueryEmbeddingService>,
    ) -> Self {
        let src_stm = Arc::new(source::StmMemorySource::new(
            cache_manager.clone(),
            storage.clone() as Arc<dyn RepositorySet>,
            registry.clone(),
        ));
        let src_ltm = Arc::new(source::LtmMemorySource::new(
            storage.clone() as Arc<dyn RepositorySet>,
            registry.clone(),
        ));
        let src_vector = Arc::new(source::SemanticMemorySource::new(
            storage.clone() as Arc<dyn RepositorySet>,
            query_embedding_service.clone(),
        ));

        // Resolve ranking strategy based on policy
        let ranking_policy = config.retrieval().ranking_policy();

        let ranking_strategy: Arc<dyn brain_core::retrieval::RankingStrategy> = match ranking_policy
        {
            brain_config::schema::RankingPolicy::LearnedModel => {
                let model_path = config.retrieval().model_path();
                let ranker = model_path
                    .ok_or_else(|| BrainError::Configuration {
                        message: "model_path must be configured for LearnedModel policy"
                            .to_string(),
                    })
                    .and_then(|path| {
                        crate::retrieval::model_loader::ModelLoader::load_from_file(path)
                    });

                match ranker {
                    Ok(score_ranker) => {
                        let reference_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let decay = ranking::feature_provider::RankingDecay {
                            recency_half_life_days: 30.0,
                            freshness_half_life_days: 90.0,
                        };
                        let provider =
                            Arc::new(ranking::feature_provider::SqliteFeatureProvider::new(
                                storage.pool().clone(),
                            ));
                        let embedding_lookup = Arc::new(RepositoryEmbeddingLookup {
                            repos: storage.clone() as Arc<dyn RepositorySet>,
                        });
                        Arc::new(ranking::model_strategy::ModelRankingStrategy::new(
                            score_ranker,
                            provider,
                            query_embedding_service.clone(),
                            embedding_lookup,
                            storage.pool().clone(),
                            reference_time,
                            decay,
                        ))
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to load learned model ranking strategy: {:?}. Falling back to DefaultRrf.",
                            e
                        );
                        let strategy_bm25 = Arc::new(ranking::Bm25Ranking::default());
                        let strategy_vector = Arc::new(ranking::EmbeddingRanking::new(
                            query_embedding_service.clone(),
                            Arc::new(RepositoryEmbeddingLookup {
                                repos: storage.clone() as Arc<dyn RepositorySet>,
                            }),
                        ));
                        Arc::new(ranking::RrfRanking::new(
                            vec![(strategy_bm25, 1.0), (strategy_vector, 1.0)],
                            60.0,
                        ))
                    }
                }
            }
            brain_config::schema::RankingPolicy::DefaultRrf => {
                let strategy_bm25 = Arc::new(ranking::Bm25Ranking::default());
                let strategy_vector = Arc::new(ranking::EmbeddingRanking::new(
                    query_embedding_service.clone(),
                    Arc::new(RepositoryEmbeddingLookup {
                        repos: storage.clone() as Arc<dyn RepositorySet>,
                    }),
                ));
                Arc::new(ranking::RrfRanking::new(
                    vec![(strategy_bm25, 1.0), (strategy_vector, 1.0)],
                    60.0,
                ))
            }
        };

        let temporal_reranker = Arc::new(ranking::reranker::TemporalReranker::new());
        let pipeline = pipeline::MemoryPipelineBuilder::new()
            .register_source(src_stm)
            .register_source(src_ltm)
            .register_source(src_vector)
            .with_ranking_strategy(ranking_strategy)
            .with_policy(CacheHydrationPolicy::OnHit)
            .with_cache_manager(cache_manager)
            .with_temporal_ranking_config(config.retrieval().temporal_ranking().clone())
            .register_reranker(temporal_reranker)
            .build();

        Self {
            repos: storage as Arc<dyn RepositorySet>,
            pipeline,
        }
    }

    /// Executes the underlying hybrid search pipeline directly.
    pub fn execute_pipeline(
        &self,
        request: &RetrievalRequest,
    ) -> Result<brain_core::retrieval::RetrievalResponse, BrainError> {
        let mut response = self.pipeline.execute(request)?;
        if request.expand_relations {
            let expander = RelationshipExpander::new(self.repos.clone());
            let expanded = expander.expand(&response.nodes)?;
            response.relationships = Some(expanded);
        }
        Ok(response)
    }
}

impl RetrievalService for RetrievalServiceImpl {
    fn retrieve(
        &self,
        session_id: &SessionId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryDTO>, BrainError> {
        let request = RetrievalRequest {
            session_id: *session_id,
            query: query.to_string(),
            limit,
            exclude_ids: std::collections::HashSet::new(),
            deadline: None,
            explain: false,
            graph_depth: None,
            expand_relations: false,
            reference_time: None,
        };

        let response = self.pipeline.execute(&request)?;

        let mut results = Vec::with_capacity(response.nodes.len());
        for node in response.nodes {
            let connections = self.repos.edges().get_connections(&node.id)?;
            let dto = to_memory_dto(&node, &connections)?;
            results.push(dto);
        }

        Ok(results)
    }
}
