use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{MemorySource, MemorySourceResult, RetrievalRequest, SourceMetadata};
use brain_session::SessionCacheManager;
use std::sync::Arc;

/// Short-term memory (STM) source querying the active session cache.
pub struct StmMemorySource {
    cache_manager: Arc<SessionCacheManager>,
}

impl StmMemorySource {
    /// Creates a new StmMemorySource.
    pub fn new(cache_manager: Arc<SessionCacheManager>) -> Self {
        Self { cache_manager }
    }
}

impl MemorySource for StmMemorySource {
    fn retrieve(&self, request: &RetrievalRequest) -> Result<MemorySourceResult, BrainError> {
        let context_lock = self.cache_manager.get_or_create(request.session_id);
        let context = context_lock.read().map_err(|e| BrainError::Internal {
            message: format!("Failed to acquire cache lock: {}", e),
        })?;

        let stm_nodes = context.query(&request.query);
        let nodes = stm_nodes
            .into_iter()
            .map(|n| n.node)
            .filter(|n| !request.exclude_ids.contains(&n.id))
            .collect();

        Ok(MemorySourceResult {
            nodes,
            metadata: SourceMetadata {
                source_name: "StmMemorySource",
            },
        })
    }
}

/// Long-term memory (LTM) source querying the database.
pub struct LtmMemorySource {
    repos: Arc<dyn RepositorySet>,
}

impl LtmMemorySource {
    /// Creates a new LtmMemorySource.
    pub fn new(repos: Arc<dyn RepositorySet>) -> Self {
        Self { repos }
    }
}

impl MemorySource for LtmMemorySource {
    fn retrieve(&self, request: &RetrievalRequest) -> Result<MemorySourceResult, BrainError> {
        let db_nodes = self.repos.nodes().list_all()?;
        let query_lower = request.query.to_lowercase();

        let nodes = db_nodes
            .into_iter()
            .filter(|node| {
                !request.exclude_ids.contains(&node.id)
                    && node.label.to_lowercase().contains(&query_lower)
            })
            .collect();

        Ok(MemorySourceResult {
            nodes,
            metadata: SourceMetadata {
                source_name: "LtmMemorySource",
            },
        })
    }
}
