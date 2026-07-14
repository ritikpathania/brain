use brain_core::errors::BrainError;
use brain_domain::NodeId;
use brain_services::eval_harness::{
    FeatureExtractor, FeatureVector, LinearRanker, RankingRetriever, RankingWeights,
    RetrievalChannel, RetrievalResult, Retriever, run_benchmark,
};
use std::collections::HashMap;

struct MockRetriever {
    results: Vec<RetrievalResult>,
}

impl Retriever for MockRetriever {
    fn retrieve(&self, _query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        Ok(self.results.clone())
    }
}

#[test]
fn test_linear_ranker_scoring() {
    let weights = RankingWeights {
        lexical: 0.6,
        semantic: 0.4,
    };
    let ranker = LinearRanker::new(weights);

    let features = FeatureVector {
        lexical_similarity: Some(10.0),
        semantic_similarity: Some(0.8),
        recency: None,
        importance: None,
        provenance_confidence: None,
        graph_distance: None,
    };

    let score = ranker.score(&features);
    // 10.0 * 0.6 + 0.8 * 0.4 = 6.0 + 0.32 = 6.32
    assert!((score - 6.32).abs() < 1e-9);
}

#[test]
fn test_ranking_retriever_sorting() {
    let node_a = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-00000000000a").unwrap());
    let node_b = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-00000000000b").unwrap());

    // Candidates before ranking
    let candidates = vec![
        RetrievalResult {
            node_id: node_a,
            channel_scores: HashMap::from([
                (RetrievalChannel::Fts, 5.0),
                (RetrievalChannel::Semantic, 0.5),
            ]),
            ranking_score: None,
        },
        RetrievalResult {
            node_id: node_b,
            channel_scores: HashMap::from([
                (RetrievalChannel::Fts, 10.0),
                (RetrievalChannel::Semantic, 0.8),
            ]),
            ranking_score: None,
        },
    ];

    let mock = MockRetriever { results: candidates };
    let weights = RankingWeights {
        lexical: 1.0,
        semantic: 2.0,
    };
    let ranker = LinearRanker::new(weights);
    let ranking_retriever = RankingRetriever::new(mock, ranker);

    let results = ranking_retriever.retrieve("query").unwrap();
    assert_eq!(results.len(), 2);

    // Node B score: 10.0 * 1.0 + 0.8 * 2.0 = 11.6
    // Node A score: 5.0 * 1.0 + 0.5 * 2.0 = 6.0
    // Node B has a higher score so it should be first
    assert_eq!(results[0].node_id, node_b);
    assert_eq!(results[0].ranking_score, Some(11.6));
    assert_eq!(results[1].node_id, node_a);
    assert_eq!(results[1].ranking_score, Some(6.0));
}

#[test]
fn test_ranking_retriever_harness_integration() {
    // Queries corpus
    let queries_json = r#"{
        "version": 1,
        "queries": [
            {
                "query_id": "q_001",
                "text": "test retrieval",
                "tags": ["unit"]
            }
        ]
    }"#;

    let node_a_str = "00000000-0000-0000-0000-00000000000a";
    let node_b_str = "00000000-0000-0000-0000-00000000000b";

    let node_a = NodeId(uuid::Uuid::parse_str(node_a_str).unwrap());
    let node_b = NodeId(uuid::Uuid::parse_str(node_b_str).unwrap());

    // Ground truth corpus: node_b is expected target, node_a is acceptable alternative
    let ground_truth_json = format!(r#"{{
        "version": 1,
        "nodes": [
            {{
                "node_id": "{}",
                "content": "rust concept",
                "type": "Concept"
            }},
            {{
                "node_id": "{}",
                "content": "python concept",
                "type": "Concept"
            }}
        ],
        "ground_truth": {{
            "q_001": {{
                "expected_node_ids": ["{}"],
                "acceptable_alternatives": ["{}"]
            }}
        }}
    }}"#, node_b_str, node_a_str, node_b_str, node_a_str);

    // Mock candidates returned by candidate generation (retriever)
    let candidates = vec![
        RetrievalResult {
            node_id: node_a,
            channel_scores: HashMap::from([
                (RetrievalChannel::Fts, 10.0),
                (RetrievalChannel::Semantic, 0.5),
            ]),
            ranking_score: None,
        },
        RetrievalResult {
            node_id: node_b,
            channel_scores: HashMap::from([
                (RetrievalChannel::Fts, 8.0),
                (RetrievalChannel::Semantic, 0.9),
            ]),
            ranking_score: None,
        },
    ];

    let mock = MockRetriever { results: candidates };
    // Let's calibrate weights so Node B wins
    // Node A FTS=10.0, Semantic=0.5 -> Score = 10.0 * 0.1 + 0.5 * 10.0 = 1.0 + 5.0 = 6.0
    // Node B FTS=8.0,  Semantic=0.9 -> Score = 8.0 * 0.1 + 0.9 * 10.0 = 0.8 + 9.0 = 9.8
    let weights = RankingWeights {
        lexical: 0.1,
        semantic: 10.0,
    };
    let ranker = LinearRanker::new(weights);
    let ranking_retriever = RankingRetriever::new(mock, ranker);

    let report = run_benchmark(queries_json, &ground_truth_json, &ranking_retriever, "cold").unwrap();

    // Check aggregate and query metrics
    assert_eq!(report.stable.metrics.total_queries, 1);
    assert_eq!(report.stable.metrics.successful_queries, 1);

    // Node B is at index 0 (ranked first), which is expected.
    // NDCG@1 = 1.0 / log2(2) = 1.0. NDCG@5 = 1.0.
    assert!((report.stable.metrics.mean_ndcg_at_5 - 1.0).abs() < 1e-9);
    assert!((report.stable.metrics.mean_ndcg_at_10 - 1.0).abs() < 1e-9);

    let q_res = &report.stable.query_results[0];
    assert!((q_res.ndcg_at_5 - 1.0).abs() < 1e-9);
    assert!((q_res.ndcg_at_10 - 1.0).abs() < 1e-9);

    // Diagnostics should include ranked score details
    let q_diag = &report.measured.diagnostics[0];
    assert_eq!(q_diag.candidates.len(), 2);

    let cand_b = q_diag.candidates.iter().find(|c| c.node_id == node_b_str).unwrap();
    assert_eq!(cand_b.ranked_score, Some(9.8));
    assert_eq!(cand_b.lexical_score, Some(8.0));
    assert_eq!(cand_b.semantic_score, Some(0.9));

    let cand_a = q_diag.candidates.iter().find(|c| c.node_id == node_a_str).unwrap();
    assert_eq!(cand_a.ranked_score, Some(6.0));
    assert_eq!(cand_a.lexical_score, Some(10.0));
    assert_eq!(cand_a.semantic_score, Some(0.5));
}
