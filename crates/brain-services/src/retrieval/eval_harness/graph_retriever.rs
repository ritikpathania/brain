use crate::retrieval::eval_harness::{RetrievalChannel, RetrievalResult, Retriever};
use crate::retrieval::source::LtmMemorySource;
use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{MemorySource, RetrievalRequest};
use brain_domain::SessionId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// A Retriever implementation wrapping LtmMemorySource for graph-aware evaluations.
pub struct GraphAwareRetriever {
    ltm_source: LtmMemorySource,
    graph_depth: usize,
}

impl GraphAwareRetriever {
    /// Creates a new `GraphAwareRetriever` from storage and relation registry.
    pub fn new(
        repos: Arc<dyn RepositorySet>,
        registry: Arc<brain_domain::RelationRegistry>,
        graph_depth: usize,
    ) -> Self {
        let ltm_source = LtmMemorySource::new(repos, registry);
        Self {
            ltm_source,
            graph_depth,
        }
    }
}

impl Retriever for GraphAwareRetriever {
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        let req = RetrievalRequest {
            session_id: SessionId::new(),
            query: query.to_string(),
            limit: 50,
            exclude_ids: HashSet::new(),
            deadline: None,
            explain: false,
            graph_depth: Some(self.graph_depth),
            expand_relations: false,
            reference_time: None,
        };

        let source_result = self.ltm_source.retrieve(&req)?;

        // Map retrieved nodes to RetrievalResults.
        // Since MemorySourceResult only exposes the final ordered node vector,
        // we assign descending synthetic channel scores for a single channel (Fts)
        // to preserve the original LtmMemorySource sorting.
        let results = source_result
            .nodes
            .into_iter()
            .enumerate()
            .map(|(index, node)| {
                let score = 1000.0 - index as f64;
                let mut channel_scores = HashMap::new();
                channel_scores.insert(RetrievalChannel::Fts, score);

                RetrievalResult {
                    node_id: node.id,
                    channel_scores,
                    ranking_score: None,
                }
            })
            .collect();

        Ok(results)
    }

    fn normalize_query(&self, query: &str) -> Option<String> {
        Some(query.to_lowercase())
    }

    fn executed_query(&self, query: &str) -> Option<String> {
        Some(format!(
            "LtmMemorySource(graph_depth={}, query={})",
            self.graph_depth, query
        ))
    }
}
