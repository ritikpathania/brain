use crate::retrieval::eval_harness::{RetrievalResult, Retriever};
use brain_core::errors::BrainError;
use brain_domain::NodeId;
use std::collections::HashMap;

/// A hybrid retriever that combines results from lexical (FTS) and semantic retrievers.
pub struct HybridRetriever<F: Retriever, S: Retriever> {
    fts: F,
    semantic: S,
}

impl<F: Retriever, S: Retriever> HybridRetriever<F, S> {
    /// Creates a new `HybridRetriever` by combining the given lexical and semantic retrievers.
    pub fn new(fts: F, semantic: S) -> Self {
        Self { fts, semantic }
    }
}

impl<F: Retriever, S: Retriever> Retriever for HybridRetriever<F, S> {
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        let fts_results = self.fts.retrieve(query)?;
        let sem_results = self.semantic.retrieve(query)?;

        let mut merged: HashMap<
            NodeId,
            HashMap<crate::retrieval::eval_harness::RetrievalChannel, f64>,
        > = HashMap::new();

        for res in fts_results {
            merged.insert(res.node_id, res.channel_scores);
        }

        for res in sem_results {
            let entry = merged.entry(res.node_id).or_default();
            for (ch, score) in res.channel_scores {
                entry.insert(ch, score);
            }
        }

        let results = merged
            .into_iter()
            .map(|(node_id, channel_scores)| RetrievalResult {
                node_id,
                channel_scores,
                ranking_score: None,
            })
            .collect();

        Ok(results)
    }

    fn normalize_query(&self, query: &str) -> Option<String> {
        self.fts.normalize_query(query)
    }

    fn executed_query(&self, query: &str) -> Option<String> {
        let fts_exec = self.fts.executed_query(query).unwrap_or_default();
        let sem_exec = self.semantic.executed_query(query).unwrap_or_default();
        Some(format!("Hybrid(Fts={}, Semantic={})", fts_exec, sem_exec))
    }
}
