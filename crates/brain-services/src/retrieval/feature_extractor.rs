use brain_core::errors::BrainError;
use brain_core::retrieval::RetrievalRequest;
use brain_core::repositories::RepositorySet;
use brain_domain::{Node, temporal::{RecencyPolicy, TimePoint, TemporalEdge}};
use brain_domain::retrieval::features::RawFeatureVector;

/// Interface for extracting raw ranking features from nodes.
pub trait FeatureExtractor: Send + Sync {
    /// Computes raw features for candidate nodes based on search request and temporal observations.
    fn extract(
        &self,
        request: &RetrievalRequest,
        nodes: &[Node],
        temporal_edges: &[TemporalEdge],
        repos: &dyn RepositorySet,
    ) -> Result<Vec<RawFeatureVector>, BrainError>;
}

/// Concrete implementation of FeatureExtractor decoupled from concrete SQLite dependencies.
pub struct DefaultFeatureExtractor {
    reference_time: TimePoint,
    recency_policy: RecencyPolicy,
}

impl DefaultFeatureExtractor {
    /// Creates a new `DefaultFeatureExtractor`.
    pub fn new(reference_time: TimePoint, recency_policy: RecencyPolicy) -> Self {
        Self { reference_time, recency_policy }
    }
}

impl FeatureExtractor for DefaultFeatureExtractor {
    fn extract(
        &self,
        request: &RetrievalRequest,
        nodes: &[Node],
        temporal_edges: &[TemporalEdge],
        repos: &dyn RepositorySet,
    ) -> Result<Vec<RawFeatureVector>, BrainError> {
        let mut node_recency = std::collections::HashMap::new();
        let mut node_temp_count = std::collections::HashMap::new();

        for te in temporal_edges {
            let t = te.observed_at.unix_seconds();
            node_recency.entry(te.edge.source)
                .and_modify(|existing| *existing = std::cmp::max(*existing, t))
                .or_insert(t);
            node_recency.entry(te.edge.target)
                .and_modify(|existing| *existing = std::cmp::max(*existing, t))
                .or_insert(t);

            *node_temp_count.entry(te.edge.source).or_insert(0) += 1;
            *node_temp_count.entry(te.edge.target).or_insert(0) += 1;
        }

        let mut raw_vectors = Vec::with_capacity(nodes.len());
        for node in nodes {
            let semantic = crate::retrieval::source::calculate_token_overlap_score(node, &request.query) as f64;
            
            // Query graph connections via RepositorySet abstraction
            let graph = repos.edges().get_connections(&node.id)?.len() as f64;

            let obs_time = node_recency.get(&node.id).cloned().unwrap_or(0);
            let recency = self.recency_policy.compute_weight(
                1.0,
                TimePoint::from_unix_seconds(obs_time),
                self.reference_time,
            );
            let temporal = node_temp_count.get(&node.id).cloned().unwrap_or(0) as f64;

            raw_vectors.push(RawFeatureVector { semantic, graph, recency, temporal });
        }
        Ok(raw_vectors)
    }
}
