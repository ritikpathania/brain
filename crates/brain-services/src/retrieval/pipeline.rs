use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use brain_core::errors::BrainError;
use brain_core::retrieval::{
    CacheHydrationPolicy, IdentityRanking, MemorySource, RankingStrategy, RetrievalRequest,
    RetrievalResponse,
};
use brain_domain::{Node, NodeId};
use brain_session::SessionCacheManager;

/// Diagnostic information recorded per memory source query.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceDiagnostic {
    /// The number of raw nodes returned by this source.
    pub raw_count: usize,
    /// The number of unique nodes accepted from this source (after deduplication and exclusions).
    pub unique_count: usize,
    /// The execution time in milliseconds.
    pub duration_ms: f64,
}

/// Accumulator to collect candidates, track seen IDs, and record per-source diagnostics.
///
/// This is the only mutable state object inside the pipeline's execution.
#[derive(Debug, Clone)]
pub struct PipelineAccumulator {
    nodes: Vec<Node>,
    seen_ids: HashSet<NodeId>,
    diagnostics: HashMap<&'static str, SourceDiagnostic>,
    source_order: Vec<&'static str>,
}

impl PipelineAccumulator {
    /// Creates a new `PipelineAccumulator` initialized with the set of excluded node IDs.
    pub fn new(exclude_ids: HashSet<NodeId>) -> Self {
        Self {
            nodes: Vec::new(),
            seen_ids: exclude_ids,
            diagnostics: HashMap::new(),
            source_order: Vec::new(),
        }
    }

    /// Appends nodes from a memory source result, checking for uniqueness and recording diagnostics.
    pub fn add_source_results(
        &mut self,
        source_name: &'static str,
        raw_nodes: Vec<Node>,
        duration_ms: f64,
    ) {
        let raw_count = raw_nodes.len();
        let mut unique_count = 0;
        for node in raw_nodes {
            if self.seen_ids.insert(node.id) {
                self.nodes.push(node);
                unique_count += 1;
            }
        }
        self.diagnostics.insert(
            source_name,
            SourceDiagnostic {
                raw_count,
                unique_count,
                duration_ms,
            },
        );
        self.source_order.push(source_name);
    }

    /// Returns the accumulated unique nodes.
    pub fn nodes(self) -> Vec<Node> {
        self.nodes
    }

    /// Returns a reference to the accumulated unique nodes.
    pub fn nodes_ref(&self) -> &[Node] {
        &self.nodes
    }

    /// Returns the number of accumulated unique nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true if no unique nodes have been accumulated.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns a reference to the recorded diagnostics map.
    pub fn diagnostics(&self) -> &HashMap<&'static str, SourceDiagnostic> {
        &self.diagnostics
    }

    /// Returns the order in which sources were recorded.
    pub fn source_order(&self) -> &[&'static str] {
        &self.source_order
    }
}

/// The orchestrator pipeline for retrieving and ranking nodes across multiple memory sources.
pub struct RetrievalPipeline {
    sources: Vec<Arc<dyn MemorySource>>,
    ranking_strategy: Arc<dyn RankingStrategy>,
    cache_manager: Option<Arc<SessionCacheManager>>,
    policy: CacheHydrationPolicy,
}

impl RetrievalPipeline {
    /// Executes the retrieval request through the pipeline, querying memory sources,
    /// deduplicating, ranking, truncating, and performing cache hydration.
    pub fn execute(&self, request: &RetrievalRequest) -> Result<RetrievalResponse, BrainError> {
        let pipeline_start = std::time::Instant::now();
        let mut accumulator = PipelineAccumulator::new(request.exclude_ids.clone());

        for source in &self.sources {
            if accumulator.len() >= request.limit {
                break;
            }

            if let Some(deadline) = request.deadline {
                if std::time::Instant::now() >= deadline {
                    return Err(BrainError::Timeout {
                        elapsed_ms: pipeline_start.elapsed().as_millis() as u64,
                        message: "Retrieval pipeline deadline exceeded".to_string(),
                    });
                }
            }

            let start = std::time::Instant::now();
            let source_result = source.retrieve(request)?;
            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

            accumulator.add_source_results(
                source_result.metadata.source_name,
                source_result.nodes,
                duration_ms,
            );

            if let Some(deadline) = request.deadline {
                if std::time::Instant::now() >= deadline {
                    return Err(BrainError::Timeout {
                        elapsed_ms: pipeline_start.elapsed().as_millis() as u64,
                        message: "Retrieval pipeline deadline exceeded".to_string(),
                    });
                }
            }
        }

        // Rank
        let ranked_nodes = self
            .ranking_strategy
            .rank(request, accumulator.nodes())?;

        // Truncate to limit
        let mut final_nodes = ranked_nodes;
        if final_nodes.len() > request.limit {
            final_nodes.truncate(request.limit);
        }

        // Hydrate cache
        if self.policy == CacheHydrationPolicy::OnHit || self.policy == CacheHydrationPolicy::Eager {
            if let Some(ref cache_manager) = self.cache_manager {
                let ctx = cache_manager.get_or_create(request.session_id);
                let mut guard = ctx.write().unwrap();
                for node in &final_nodes {
                    guard.ingest(node.clone());
                }
            }
        }

        Ok(RetrievalResponse { nodes: final_nodes })
    }

    /// Returns a reference to the registered sources.
    pub fn sources(&self) -> &[Arc<dyn MemorySource>] {
        &self.sources
    }

    /// Returns a reference to the ranking strategy.
    pub fn ranking_strategy(&self) -> &Arc<dyn RankingStrategy> {
        &self.ranking_strategy
    }

    /// Returns a reference to the cache manager, if registered.
    pub fn cache_manager(&self) -> Option<&Arc<SessionCacheManager>> {
        self.cache_manager.as_ref()
    }

    /// Returns the cache hydration policy.
    pub fn policy(&self) -> CacheHydrationPolicy {
        self.policy
    }
}

/// A builder to construct `RetrievalPipeline` instances.
pub struct MemoryPipelineBuilder {
    sources: Vec<Arc<dyn MemorySource>>,
    ranking_strategy: Arc<dyn RankingStrategy>,
    cache_manager: Option<Arc<SessionCacheManager>>,
    policy: CacheHydrationPolicy,
}

impl Default for MemoryPipelineBuilder {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            ranking_strategy: Arc::new(IdentityRanking),
            cache_manager: None,
            policy: CacheHydrationPolicy::Never,
        }
    }
}

impl MemoryPipelineBuilder {
    /// Creates a new `MemoryPipelineBuilder` with default empty configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a memory source in the pipeline.
    pub fn register_source(mut self, source: Arc<dyn MemorySource>) -> Self {
        self.sources.push(source);
        self
    }

    /// Configures the ranking strategy for the pipeline.
    pub fn with_ranking_strategy(mut self, ranking_strategy: Arc<dyn RankingStrategy>) -> Self {
        self.ranking_strategy = ranking_strategy;
        self
    }

    /// Configures the session cache manager for the pipeline.
    pub fn with_cache_manager(mut self, cache_manager: Arc<SessionCacheManager>) -> Self {
        self.cache_manager = Some(cache_manager);
        self
    }

    /// Configures the cache hydration policy for the pipeline.
    pub fn with_policy(mut self, policy: CacheHydrationPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Builds a new `RetrievalPipeline` using the configured settings.
    pub fn build(self) -> RetrievalPipeline {
        RetrievalPipeline {
            sources: self.sources,
            ranking_strategy: self.ranking_strategy,
            cache_manager: self.cache_manager,
            policy: self.policy,
        }
    }
}
