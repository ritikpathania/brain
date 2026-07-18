use std::sync::Arc;
use std::collections::HashSet;
use brain_core::errors::BrainError;
use brain_core::repositories::{
    RepositorySet, NodeRepository, EdgeRepository, EmbeddingRepository,
    SessionRepository, ConfigRepository
};
use brain_core::retrieval::{
    RankingStrategy, RetrievalRequest, CacheHydrationPolicy
};
use brain_domain::{
    Node, Edge, EdgeId, NodeId, SessionId, MemoryDTO, Evidence,
    temporal::{TemporalSnapshot, TemporalQuery, TemporalProjector, RecencyPolicy, TimePoint, TemporalEdge}
};
use brain_storage::SqliteStorage;
use brain_session::SessionCacheManager;

/// A read-only repository decorator that filters edges based on a temporal visibility snapshot.
pub struct ProjectedRepositoryView {
    underlying: Arc<dyn RepositorySet>,
    edge_repo: ProjectedEdgeRepository,
}

impl ProjectedRepositoryView {
    /// Creates a new `ProjectedRepositoryView` wrapping the underlying repositories and filtering edges.
    pub fn new(underlying: Arc<dyn RepositorySet>, snapshot: TemporalSnapshot) -> Self {
        let edge_repo = ProjectedEdgeRepository {
            underlying: underlying.clone(),
            active_edge_ids: snapshot.active_edge_ids,
        };
        Self {
            underlying,
            edge_repo,
        }
    }
}

impl RepositorySet for ProjectedRepositoryView {
    fn nodes(&self) -> &dyn NodeRepository {
        self.underlying.nodes()
    }

    fn edges(&self) -> &dyn EdgeRepository {
        &self.edge_repo
    }

    fn embeddings(&self) -> &dyn EmbeddingRepository {
        self.underlying.embeddings()
    }

    fn sessions(&self) -> &dyn SessionRepository {
        self.underlying.sessions()
    }

    fn configs(&self) -> &dyn ConfigRepository {
        self.underlying.configs()
    }
}

/// A read-only edge repository decorator filtering out inactive edges.
pub struct ProjectedEdgeRepository {
    underlying: Arc<dyn RepositorySet>,
    active_edge_ids: HashSet<EdgeId>,
}

impl EdgeRepository for ProjectedEdgeRepository {
    fn save(&self, _edge: &Edge) -> Result<(), BrainError> {
        Err(BrainError::Storage {
            message: "Mutation operations are not supported on ProjectedRepositoryView".to_string(),
            source: None,
        })
    }

    fn save_batch(&self, _edges: &[Edge]) -> Result<(), BrainError> {
        Err(BrainError::Storage {
            message: "Mutation operations are not supported on ProjectedRepositoryView".to_string(),
            source: None,
        })
    }

    fn find_by_id(&self, id: &EdgeId) -> Result<Option<Edge>, BrainError> {
        if !self.active_edge_ids.contains(id) {
            return Ok(None);
        }
        self.underlying.edges().find_by_id(id)
    }

    fn delete(&self, _id: &EdgeId) -> Result<(), BrainError> {
        Err(BrainError::Storage {
            message: "Mutation operations are not supported on ProjectedRepositoryView".to_string(),
            source: None,
        })
    }

    fn get_connections(&self, node_id: &NodeId) -> Result<Vec<Edge>, BrainError> {
        let raw = self.underlying.edges().get_connections(node_id)?;
        let filtered = raw.into_iter().filter(|edge| {
            let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());
            self.active_edge_ids.contains(&edge_id)
        }).collect();
        Ok(filtered)
    }

    fn list_all(&self) -> Result<Vec<Edge>, BrainError> {
        let raw = self.underlying.edges().list_all()?;
        let filtered = raw.into_iter().filter(|edge| {
            let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());
            self.active_edge_ids.contains(&edge_id)
        }).collect();
        Ok(filtered)
    }
}

/// A context-aware temporal ranking strategy that applies exponential/linear decay based on edge recency.
pub struct TemporalRankingStrategy {
    base_strategy: Arc<dyn RankingStrategy>,
    recency_policy: RecencyPolicy,
    storage: Arc<SqliteStorage>,
    reference_time: TimePoint,
}

impl TemporalRankingStrategy {
    /// Creates a new `TemporalRankingStrategy` applying recency weight decays based on query constraints.
    pub fn new(
        base_strategy: Arc<dyn RankingStrategy>,
        recency_policy: RecencyPolicy,
        storage: Arc<SqliteStorage>,
        reference_time: TimePoint,
    ) -> Self {
        Self {
            base_strategy,
            recency_policy,
            storage,
            reference_time,
        }
    }
}

impl RankingStrategy for TemporalRankingStrategy {
    fn rank(
        &self,
        request: &RetrievalRequest,
        nodes: Vec<Node>,
    ) -> Result<Vec<Node>, BrainError> {
        // Run base ranking strategy to get matched nodes
        let base_ranked = self.base_strategy.rank(request, nodes)?;

        if let RecencyPolicy::None = self.recency_policy {
            return Ok(base_ranked);
        }

        // Fetch all temporal edges to lookup node observation times
        let temp_edges = self.storage.list_all_temporal_edges()?;
        let mut node_recency: std::collections::HashMap<NodeId, u64> = std::collections::HashMap::new();

        for te in &temp_edges {
            let t = te.observed_at.unix_seconds();
            node_recency.entry(te.edge.source)
                .and_modify(|existing| *existing = std::cmp::max(*existing, t))
                .or_insert(t);
            node_recency.entry(te.edge.target)
                .and_modify(|existing| *existing = std::cmp::max(*existing, t))
                .or_insert(t);
        }

        let mut scored_nodes: Vec<(Node, f64)> = base_ranked
            .into_iter()
            .map(|node| {
                // Calculate match score
                let base_score = crate::retrieval::source::calculate_token_overlap_score(&node, &request.query) as f64;
                let obs_time = node_recency.get(&node.id).cloned().unwrap_or(0);
                
                let reference_secs = self.reference_time.unix_seconds();
                let _elapsed = reference_secs.saturating_sub(obs_time) as f64;

                let decayed_score = self.recency_policy.compute_weight(
                    base_score,
                    TimePoint::from_unix_seconds(obs_time),
                    self.reference_time,
                );
                (node, decayed_score)
            })
            .collect();

        // Sort by decayed score descending, fallback deterministically to UUID comparison
        scored_nodes.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.id.0.cmp(&b.0.id.0))
        });

        let final_nodes = scored_nodes.into_iter().map(|(n, _)| n).collect();
        Ok(final_nodes)
    }
}

/// Service handling memory retrieval querying using temporal snapshots and decorators.
pub struct TemporalRetrievalService {
    storage: Arc<SqliteStorage>,
    registry: Arc<brain_domain::RelationRegistry>,
    cache_manager: Option<Arc<SessionCacheManager>>,
}

impl TemporalRetrievalService {
    /// Creates a new `TemporalRetrievalService`.
    pub fn new(
        storage: Arc<SqliteStorage>,
        registry: Arc<brain_domain::RelationRegistry>,
        cache_manager: Option<Arc<SessionCacheManager>>,
    ) -> Self {
        Self {
            storage,
            registry,
            cache_manager,
        }
    }

    /// Evaluates and runs memory retrieval under target historical/visibility constraints.
    pub fn retrieve_temporal(
        &self,
        session_id: &SessionId,
        query: &str,
        limit: usize,
        temporal_query: &TemporalQuery,
    ) -> Result<(Vec<MemoryDTO>, std::collections::HashMap<NodeId, Vec<Evidence>>), BrainError> {
        let request = RetrievalRequest {
            session_id: *session_id,
            query: query.to_string(),
            limit,
            exclude_ids: std::collections::HashSet::new(),
            deadline: None,
        };

        // 1. Load all temporal edges from SQLite storage
        let temporal_edges = self.storage.list_all_temporal_edges()?;

        // 2. Perform projection using pure TemporalProjector
        let snapshot = TemporalProjector::project(&temporal_edges, temporal_query);

        // 3. Wrap storage in ProjectedRepositoryView decorator
        let projected_repos = Arc::new(ProjectedRepositoryView::new(
            self.storage.clone() as Arc<dyn RepositorySet>,
            snapshot.clone(),
        ));

        // 4. Instantiate memory sources referencing the projected repository
        let ltm_source = Arc::new(crate::retrieval::source::LtmMemorySource::new(
            projected_repos.clone() as Arc<dyn RepositorySet>,
            self.registry.clone(),
        ));

        let mut builder = crate::retrieval::pipeline::MemoryPipelineBuilder::new()
            .register_source(ltm_source);

        if let Some(ref cache_manager) = self.cache_manager {
            builder = builder
                .with_cache_manager(cache_manager.clone())
                .register_source(Arc::new(crate::retrieval::source::StmMemorySource::new(
                    cache_manager.clone(),
                    projected_repos.clone() as Arc<dyn RepositorySet>,
                    self.registry.clone(),
                )));
        }

        // Base ranking strategy
        let base_ranking = Arc::new(brain_core::retrieval::IdentityRanking);

        // Decorate with recency preference ranking
        let ranking = Arc::new(TemporalRankingStrategy::new(
            base_ranking,
            temporal_query.recency_policy,
            self.storage.clone(),
            temporal_query.reference_time,
        ));

        let pipeline = builder
            .with_ranking_strategy(ranking)
            .with_policy(CacheHydrationPolicy::OnHit)
            .build();

        let response = pipeline.execute(&request)?;

        // Map retrieved nodes and build explanations
        let mut results = Vec::with_capacity(response.nodes.len());
        let mut explanations = std::collections::HashMap::new();

        // Build node recency map to explain decay rankings
        let mut node_recency: std::collections::HashMap<NodeId, u64> = std::collections::HashMap::new();
        for te in &temporal_edges {
            let t = te.observed_at.unix_seconds();
            node_recency.entry(te.edge.source)
                .and_modify(|existing| *existing = std::cmp::max(*existing, t))
                .or_insert(t);
            node_recency.entry(te.edge.target)
                .and_modify(|existing| *existing = std::cmp::max(*existing, t))
                .or_insert(t);
        }

        for node in response.nodes {
            let connections = projected_repos.edges().get_connections(&node.id)?;
            let dto = crate::mapper::to_memory_dto(&node, &connections)?;
            results.push(dto);

            // Build node explainability trail
            let mut node_evidences = Vec::new();

            // Part A: Temporal Visibility explanations
            let mut active_connected_edges: Vec<&TemporalEdge> = temporal_edges
                .iter()
                .filter(|te| {
                    (te.edge.source == node.id || te.edge.target == node.id)
                        && snapshot.active_edge_ids.contains(&EdgeId::new(te.edge.source, te.edge.target, te.edge.relation.id()))
                })
                .collect();

            // Enforce strictly deterministic ordering of visibility evidence
            active_connected_edges.sort_by(|a, b| {
                a.edge.source.0.cmp(&b.edge.source.0)
                    .then_with(|| a.edge.target.0.cmp(&b.edge.target.0))
                    .then_with(|| a.edge.relation.id().as_str().cmp(b.edge.relation.id().as_str()))
            });

            for te in active_connected_edges {
                let visibility = brain_domain::query::HistoricalExplanationBuilder::build_visibility_evidence(te, temporal_query);
                node_evidences.push(visibility);
            }

            // Part B: Recency decay preference explanations
            if let Some(obs_time) = node_recency.get(&node.id) {
                let eval = brain_domain::query::RecencyEvaluation::new(
                    temporal_query.recency_policy,
                    TimePoint::from_unix_seconds(*obs_time),
                    temporal_query.reference_time,
                );

                // Add recency decay evidence block only if recency policy is active
                if let RecencyPolicy::None = temporal_query.recency_policy {
                    // Do not append RecencyDecay for None policy to satisfy exact completeness mapping
                } else {
                    let decay_evidence = brain_domain::query::HistoricalExplanationBuilder::build_decay_evidence(&eval);
                    node_evidences.push(decay_evidence);
                }
            }

            explanations.insert(node.id, node_evidences);
        }

        Ok((results, explanations))
    }
}

/// A deterministic, model-based temporal ranking strategy that weights multiple features.
pub struct LearnedTemporalScorer {
    weight_provider: Arc<dyn crate::retrieval::experiment::ExperimentRouter>,
    storage: Arc<SqliteStorage>,
    reference_time: TimePoint,
    recency_policy: RecencyPolicy,
}

impl LearnedTemporalScorer {
    /// Creates a new `LearnedTemporalScorer` using the active weight provider.
    pub fn new(
        weight_provider: Arc<dyn crate::retrieval::experiment::ExperimentRouter>,
        storage: Arc<SqliteStorage>,
        reference_time: TimePoint,
        recency_policy: RecencyPolicy,
    ) -> Self {
        Self {
            weight_provider,
            storage,
            reference_time,
            recency_policy,
        }
    }
}

impl RankingStrategy for LearnedTemporalScorer {
    fn rank(&self, request: &RetrievalRequest, nodes: Vec<Node>) -> Result<Vec<Node>, BrainError> {
        if nodes.is_empty() {
            return Ok(nodes);
        }

        use brain_domain::retrieval::features::{MinMaxNormalizer, FeatureNormalizer, NormalizationContext};
        use crate::retrieval::feature_extractor::{FeatureExtractor, DefaultFeatureExtractor};

        // 1. Fetch temporal edges and project snapshot
        let temp_edges = self.storage.list_all_temporal_edges()?;
        let snapshot = brain_domain::temporal::TemporalProjector::project(&temp_edges, &brain_domain::temporal::TemporalQuery {
            reference_time: self.reference_time,
            visibility: brain_domain::temporal::TemporalVisibility::Historical,
            recency_policy: self.recency_policy.clone(),
        });
        let projected_repos = ProjectedRepositoryView::new(
            self.storage.clone() as Arc<dyn RepositorySet>,
            snapshot,
        );

        // 2. Extract raw features using FeatureExtractor
        let extractor = DefaultFeatureExtractor::new(
            self.reference_time,
            self.recency_policy.clone(),
        );
        let raw_features = extractor.extract(request, &nodes, &temp_edges, &projected_repos)?;

        // 3. Normalize features using FeatureNormalizer & NormalizationContext
        let normalizer = MinMaxNormalizer;
        let context = NormalizationContext::BatchMinMax;
        let normalized_signals = normalizer.normalize(&raw_features, &context)
            .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?;

        // 4. Load active weights model and score nodes
        let routing_decision = self.weight_provider.route_decision(request)?;
        let model = crate::retrieval::model_resolver::ModelDeserializer::resolve(&routing_decision.snapshot)?;

        let mut scored_nodes = Vec::with_capacity(nodes.len());
        for (idx, node) in nodes.into_iter().enumerate() {
            let score = model.score(&normalized_signals[idx]);
            scored_nodes.push((node, score));
        }

        // 5. Sort descending, fallback to ID
        scored_nodes.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.id.0.cmp(&b.0.id.0))
        });

        Ok(scored_nodes.into_iter().map(|(n, _)| n).collect())
    }
}
