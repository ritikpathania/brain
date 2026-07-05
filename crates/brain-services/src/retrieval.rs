/// Memory retrieval orchestrator pipeline, accumulator, and builder.
pub mod pipeline;
/// Contextual ranking strategies (BM25, Embeddings, Graph, and RRF).
pub mod ranking;
/// Concrete memory source implementations (STM, LTM, etc).
pub mod source;
/// Graph traversal, budgeting, and analysis services.
pub mod graph_service;
/// Cache layers and snapshot execution management.
pub mod cache;
/// Cost calibration and feedback engine.
pub mod calibration;
/// Durable SQLite cache store backend.
pub mod sqlite_store;
/// Temporal retrieval integration, projection views, and ranking.
pub mod temporal;
/// Active weights snapshot provider abstractions.
pub mod active_weights;
/// Feature extraction pipeline for learned ranking.
pub mod feature_extractor;

use crate::mapper::to_memory_dto;
use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{CacheHydrationPolicy, RetrievalRequest};
use brain_core::services::RetrievalService;
use brain_domain::{MemoryDTO, SessionId};
use brain_session::SessionCacheManager;
use std::sync::Arc;

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
    ) -> Self {
        let pipeline = pipeline::MemoryPipelineBuilder::new()
            .register_source(Arc::new(source::StmMemorySource::new(
                cache_manager.clone(),
                repos.clone(),
                registry.clone(),
            )))
            .register_source(Arc::new(source::LtmMemorySource::new(repos.clone(), registry)))
            .with_policy(CacheHydrationPolicy::OnHit)
            .with_cache_manager(cache_manager)
            .build();

        Self { repos, pipeline }
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
