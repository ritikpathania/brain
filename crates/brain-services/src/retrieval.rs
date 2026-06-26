/// Memory retrieval orchestrator pipeline, accumulator, and builder.
pub mod pipeline;
/// Concrete memory source implementations (STM, LTM, etc).
pub mod source;
/// Contextual ranking strategies (BM25, Embeddings, Graph, and RRF).
pub mod ranking;

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
    pub fn new(repos: Arc<dyn RepositorySet>, cache_manager: Arc<SessionCacheManager>) -> Self {
        let pipeline = pipeline::MemoryPipelineBuilder::new()
            .register_source(Arc::new(source::StmMemorySource::new(
                cache_manager.clone(),
            )))
            .register_source(Arc::new(source::LtmMemorySource::new(repos.clone())))
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
