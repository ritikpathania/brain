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
    rerankers: Vec<Arc<dyn brain_core::retrieval::Reranker>>,
    temporal_ranking_config: brain_core::retrieval::TemporalRankingSettings,
}

impl RetrievalPipeline {
    /// Executes the retrieval request through the pipeline.
    ///
    /// The pipeline is organized into five ordered stages:
    ///
    /// 1. **Normalization** — the query string is interpreted as-is by each memory
    ///    source; sources are responsible for their own query preprocessing.
    /// 2. **Candidate Retrieval** — each registered `MemorySource` is queried in
    ///    order. Results are deduplicated into a flat candidate pool via the
    ///    `PipelineAccumulator`.
    /// 3. **Fusion / Ranking** — the accumulated candidates are passed to the
    ///    registered `RankingStrategy` (e.g. RRF, BM25, embedding cosine) which
    ///    produces the final ordered list.
    /// 4. **Truncation** — the ranked list is capped to `request.limit`.
    /// 5. **Projection** — downstream callers (e.g. `SearchProjector`) transform the
    ///    ranked `Node` list into presentation-ready DTOs. This boundary is NOT inside
    ///    `RetrievalPipeline`; it is the responsibility of the calling layer.
    pub fn execute(&self, request: &RetrievalRequest) -> Result<RetrievalResponse, BrainError> {
        let pipeline_start = std::time::Instant::now();

        // ── Stage 1: Normalization (delegated to each MemorySource) ─────────────
        let mut accumulator = PipelineAccumulator::new(request.exclude_ids.clone());

        // ── Stage 2: Candidate Retrieval ─────────────────────────────────────────
        for source in &self.sources {
            if self.sources.len() > 1 {
                if accumulator.len() >= request.limit * 3 {
                    break;
                }
            } else if accumulator.len() >= request.limit {
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

            tracing::info!(
                target: "brain::telemetry::retrieval",
                stage = "candidate_counts",
                source_name = source_result.metadata.source_name,
                count = source_result.nodes.len(),
                duration_ms = duration_ms,
                "Memory source returned candidates"
            );

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

        // ── Stage 3: Fusion / Ranking ─────────────────────────────────────────────
        let ranked_nodes = self.ranking_strategy.rank(request, accumulator.nodes())?;

        // ── Stage 3.5: Reranking ──────────────────────────────────────────────────
        let reference_time = request.reference_time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        });
        let rerank_context = brain_core::retrieval::RerankContext {
            request,
            config: &self.temporal_ranking_config,
            reference_time,
        };
        let mut reranked_nodes = ranked_nodes;
        for reranker in &self.rerankers {
            reranked_nodes = reranker.rerank(reranked_nodes, &rerank_context)?;
        }

        // ── Stage 4: Truncation ───────────────────────────────────────────────────
        let mut final_nodes = reranked_nodes;
        if final_nodes.len() > request.limit {
            final_nodes.truncate(request.limit);
        }

        // Cache hydration (side-effect; not a pipeline stage proper)
        if self.policy == CacheHydrationPolicy::OnHit || self.policy == CacheHydrationPolicy::Eager
        {
            if let Some(ref cache_manager) = self.cache_manager {
                let ctx = cache_manager.get_or_create(request.session_id);
                let mut guard = ctx.write().unwrap();
                for node in &final_nodes {
                    guard.ingest(node.clone());
                }
            }
        }

        let total_duration = pipeline_start.elapsed();
        tracing::info!(
            target: "brain::telemetry::retrieval",
            stage = "pipeline",
            duration_ms = total_duration.as_millis(),
            total_candidate_count = final_nodes.len(),
            "Retrieval pipeline execution completed"
        );

        // ── Stage 5: Projection ─── (performed by the calling layer, not here) ───
        Ok(RetrievalResponse {
            nodes: final_nodes,
            explanation: None,
            relationships: None,
        })
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
    rerankers: Vec<Arc<dyn brain_core::retrieval::Reranker>>,
    temporal_ranking_config: brain_core::retrieval::TemporalRankingSettings,
}

impl Default for MemoryPipelineBuilder {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            ranking_strategy: Arc::new(IdentityRanking),
            cache_manager: None,
            policy: CacheHydrationPolicy::Never,
            rerankers: Vec::new(),
            temporal_ranking_config: brain_core::retrieval::TemporalRankingSettings {
                enabled: false,
                model: brain_core::retrieval::DecayModel::Uniform,
                half_life_seconds: 86400,
                scaling_factor: 1.0,
            },
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

    /// Configures the temporal ranking settings for the pipeline.
    pub fn with_temporal_ranking_config(
        mut self,
        config: brain_core::retrieval::TemporalRankingSettings,
    ) -> Self {
        self.temporal_ranking_config = config;
        self
    }

    /// Registers a reranker in the pipeline.
    pub fn register_reranker(mut self, reranker: Arc<dyn brain_core::retrieval::Reranker>) -> Self {
        self.rerankers.push(reranker);
        self
    }

    /// Builds a new `RetrievalPipeline` using the configured settings.
    pub fn build(self) -> RetrievalPipeline {
        RetrievalPipeline {
            sources: self.sources,
            ranking_strategy: self.ranking_strategy,
            cache_manager: self.cache_manager,
            policy: self.policy,
            rerankers: self.rerankers,
            temporal_ranking_config: self.temporal_ranking_config,
        }
    }
}
