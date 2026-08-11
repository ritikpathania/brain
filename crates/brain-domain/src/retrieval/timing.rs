//! Structured retrieval execution timing metrics.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Detailed breakdown of retrieval execution timings across pipeline passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RetrievalTiming {
    /// Total wall-clock duration of the retrieval request.
    pub total: Duration,
    /// Duration spent computing dense vector embeddings.
    pub embedding: Duration,
    /// Duration spent executing FTS5 keyword index search.
    pub keyword_search: Duration,
    /// Duration spent querying vector similarity store.
    pub vector_search: Duration,
    /// Duration spent in candidate fusion and reranking.
    pub reranking: Duration,
}

impl RetrievalTiming {
    /// Convenience helper calculating total latency in milliseconds.
    pub fn total_ms(&self) -> u64 {
        self.total.as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrieval_timing_ms() {
        let timing = RetrievalTiming {
            total: Duration::from_millis(45),
            embedding: Duration::from_millis(12),
            keyword_search: Duration::from_millis(8),
            vector_search: Duration::from_millis(15),
            reranking: Duration::from_millis(10),
        };
        assert_eq!(timing.total_ms(), 45);
    }
}
