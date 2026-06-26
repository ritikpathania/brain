/// Memory retrieval orchestrator pipeline, accumulator, and builder.
pub mod pipeline;
/// Concrete memory source implementations (STM, LTM, etc).
pub mod source;

use crate::mapper::to_memory_dto;
use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::services::RetrievalService;
use brain_domain::{MemoryDTO, SessionId};
use brain_session::SessionCacheManager;
use std::sync::Arc;

/// Concrete implementation of RetrievalService routing query searches across cache and storage.
pub struct RetrievalServiceImpl {
    repos: Arc<dyn RepositorySet>,
    cache_manager: Arc<SessionCacheManager>,
}

impl RetrievalServiceImpl {
    /// Creates a new RetrievalServiceImpl.
    pub fn new(repos: Arc<dyn RepositorySet>, cache_manager: Arc<SessionCacheManager>) -> Self {
        Self {
            repos,
            cache_manager,
        }
    }

    fn retrieve_ltm(
        &self,
        session_id: &SessionId,
        query: &str,
        limit: usize,
        exclude_ids: &[String],
    ) -> Result<Vec<MemoryDTO>, BrainError> {
        let mut results = Vec::new();
        let db_nodes = self.repos.nodes().list_all()?;
        let query_lower = query.to_lowercase();

        for node in db_nodes {
            let id_str = node.id.to_string();
            // Skip if already found in STM cache
            if exclude_ids.contains(&id_str) {
                continue;
            }

            // Simple keyword match on label
            if node.label.to_lowercase().contains(&query_lower) {
                let connections = self.repos.edges().get_connections(&node.id)?;

                // Ingest the DB hit into STM cache
                let ctx = self.cache_manager.get_or_create(*session_id);
                ctx.write().unwrap().ingest(node.clone());

                let dto = to_memory_dto(&node, &connections)?;
                results.push(dto);
                if results.len() >= limit {
                    break;
                }
            }
        }

        Ok(results)
    }
}

impl RetrievalService for RetrievalServiceImpl {
    fn retrieve(
        &self,
        session_id: &SessionId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryDTO>, BrainError> {
        let mut results = Vec::new();
        let mut stm_ids = Vec::new();

        // 1. Query short-term memory cache
        let stm_nodes = {
            let ctx = self.cache_manager.get_or_create(*session_id);
            let nodes = ctx.read().unwrap().query(query);
            nodes
        };

        for stm in stm_nodes {
            let connections = self.repos.edges().get_connections(&stm.node.id)?;
            let dto = to_memory_dto(&stm.node, &connections)?;
            stm_ids.push(stm.node.id.to_string());
            results.push(dto);
            if results.len() >= limit {
                break;
            }
        }

        // 2. If limit is not reached, fall back to scanning long-term database storage
        if results.len() < limit {
            let ltm_limit = limit - results.len();
            let mut ltm_results = self.retrieve_ltm(session_id, query, ltm_limit, &stm_ids)?;
            results.append(&mut ltm_results);
        }

        results.truncate(limit);
        Ok(results)
    }
}
