use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::EmbeddingProvider;
use brain_domain::{Embedding, Node, NodeId, NodeType};
use brain_services::eval_harness::{
    sort_results_deterministically, HybridRetriever, RetrievalChannel, RetrievalResult, Retriever,
    SemanticRetriever,
};
use brain_storage::TestStorage;
use std::sync::Arc;

struct MockEmbeddingProvider {
    vector: Vec<f32>,
}

impl EmbeddingProvider for MockEmbeddingProvider {
    fn name(&self) -> &'static str {
        "mock-provider"
    }

    fn embed(&self, _text: &str) -> Result<Vec<f32>, BrainError> {
        Ok(self.vector.clone())
    }
}

#[test]
fn test_semantic_retriever_cosine_similarity() {
    let test_storage = TestStorage::new();
    let sqlite = test_storage.storage();

    let node_id_1 = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let node_id_2 = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap());

    // Save nodes
    sqlite
        .nodes()
        .save(&Node::new(
            node_id_1,
            "rust code".to_string(),
            NodeType::Concept,
        ))
        .unwrap();
    sqlite
        .nodes()
        .save(&Node::new(
            node_id_2,
            "python code".to_string(),
            NodeType::Concept,
        ))
        .unwrap();

    // Save embeddings (dimension = 3)
    sqlite
        .embeddings()
        .save(&Embedding::new(node_id_1, vec![1.0, 0.0, 0.0]))
        .unwrap();
    sqlite
        .embeddings()
        .save(&Embedding::new(node_id_2, vec![0.0, 1.0, 0.0]))
        .unwrap();

    // Provider returns vector aligned with node_id_1
    let provider = Arc::new(MockEmbeddingProvider {
        vector: vec![1.0, 0.0, 0.0],
    });

    let semantic_retriever = SemanticRetriever::new(sqlite.pool().clone(), provider);
    let mut results = semantic_retriever.retrieve("query").unwrap();

    // Sort to verify deterministic scores
    sort_results_deterministically(&mut results);

    assert_eq!(results.len(), 2);
    // Node 1 should be first because it is perfectly aligned (cosine similarity = 1.0)
    assert_eq!(results[0].node_id, node_id_1);
    assert_eq!(results[0].score(RetrievalChannel::Semantic).unwrap(), 1.0);

    // Node 2 should be perpendicular (cosine similarity = 0.0)
    assert_eq!(results[1].node_id, node_id_2);
    assert_eq!(results[1].score(RetrievalChannel::Semantic).unwrap(), 0.0);
}

struct MockFtsRetriever {
    results: Vec<RetrievalResult>,
}
impl Retriever for MockFtsRetriever {
    fn retrieve(&self, _query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        Ok(self.results.clone())
    }
    fn normalize_query(&self, _query: &str) -> Option<String> {
        Some("normalized_fts".to_string())
    }
    fn executed_query(&self, _query: &str) -> Option<String> {
        Some("FtsExec".to_string())
    }
}

struct MockSemanticRetriever {
    results: Vec<RetrievalResult>,
}
impl Retriever for MockSemanticRetriever {
    fn retrieve(&self, _query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        Ok(self.results.clone())
    }
    fn normalize_query(&self, _query: &str) -> Option<String> {
        Some("normalized_sem".to_string())
    }
    fn executed_query(&self, _query: &str) -> Option<String> {
        Some("SemExec".to_string())
    }
}

#[test]
fn test_hybrid_candidate_union_verification() {
    let node_a = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-00000000000a").unwrap());
    let node_b = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-00000000000b").unwrap());
    let node_c = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-00000000000c").unwrap());

    // Mock FTS returns Node A, Node B
    let fts_mock = MockFtsRetriever {
        results: vec![
            RetrievalResult {
                node_id: node_a,
                channel_scores: std::collections::HashMap::from([(RetrievalChannel::Fts, 8.4)]),
                ranking_score: None,
            },
            RetrievalResult {
                node_id: node_b,
                channel_scores: std::collections::HashMap::from([(RetrievalChannel::Fts, 6.2)]),
                ranking_score: None,
            },
        ],
    };

    // Mock Semantic returns Node B, Node C
    let sem_mock = MockSemanticRetriever {
        results: vec![
            RetrievalResult {
                node_id: node_b,
                channel_scores: std::collections::HashMap::from([(
                    RetrievalChannel::Semantic,
                    0.92,
                )]),
                ranking_score: None,
            },
            RetrievalResult {
                node_id: node_c,
                channel_scores: std::collections::HashMap::from([(
                    RetrievalChannel::Semantic,
                    0.75,
                )]),
                ranking_score: None,
            },
        ],
    };

    let hybrid = HybridRetriever::new(fts_mock, sem_mock);
    let mut results = hybrid.retrieve("query").unwrap();

    // Sort to verify deterministic fallback ordering (sorting ascending by NodeId)
    sort_results_deterministically(&mut results);

    assert_eq!(results.len(), 3);

    // Node A is FTS-only
    let res_a = results.iter().find(|r| r.node_id == node_a).unwrap();
    assert_eq!(res_a.channels(), vec![RetrievalChannel::Fts]);
    assert_eq!(res_a.score(RetrievalChannel::Fts), Some(8.4));
    assert_eq!(res_a.score(RetrievalChannel::Semantic), None);

    // Node B is retrieved by BOTH FTS and Semantic
    let res_b = results.iter().find(|r| r.node_id == node_b).unwrap();
    assert_eq!(
        res_b.channels(),
        vec![RetrievalChannel::Fts, RetrievalChannel::Semantic]
    );
    assert_eq!(res_b.score(RetrievalChannel::Fts), Some(6.2));
    assert_eq!(res_b.score(RetrievalChannel::Semantic), Some(0.92));

    // Node C is Semantic-only
    let res_c = results.iter().find(|r| r.node_id == node_c).unwrap();
    assert_eq!(res_c.channels(), vec![RetrievalChannel::Semantic]);
    assert_eq!(res_c.score(RetrievalChannel::Fts), None);
    assert_eq!(res_c.score(RetrievalChannel::Semantic), Some(0.75));

    // Verify correct sorting (ascending by NodeId for hybrid candidate union)
    assert_eq!(results[0].node_id, node_a);
    assert_eq!(results[1].node_id, node_b);
    assert_eq!(results[2].node_id, node_c);

    // Verify executed query combination
    assert_eq!(
        hybrid.executed_query("query").unwrap(),
        "Hybrid(Fts=FtsExec, Semantic=SemExec)"
    );
}
