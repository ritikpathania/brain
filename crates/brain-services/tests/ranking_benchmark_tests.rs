use brain_core::errors::BrainError;
use brain_core::RepositorySet;
use brain_domain::{Edge, Node, NodeId, NodeType, RelationKind};
use brain_services::eval_harness::{
    run_benchmark, FeatureContext, FeatureExtractor, FeatureProvider, FeatureVector, LinearRanker,
    RankingDecay, RankingRetriever, RankingWeights, RetrievalChannel, RetrievalResult, Retriever,
};
use brain_storage::TestStorage;
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
        lexical: 0.5,
        semantic: 0.5,
        recency: 1.0,
        importance: 2.0,
        provenance_confidence: 0.5,
        graph_degree: 1.5,
        access_frequency: 1.0,
        freshness_decay: 2.0,
    };
    let ranker = LinearRanker::new(weights);

    let features = FeatureVector {
        lexical_similarity: Some(10.0),
        semantic_similarity: Some(0.8),
        recency: Some(0.9),
        importance: Some(0.5),
        provenance_confidence: Some(0.8),
        graph_degree: Some(1.2),
        access_frequency: Some(0.7),
        freshness_decay: Some(0.6),
    };

    let score = ranker.score(&features);
    // Calculations:
    // 10.0*0.5 + 0.8*0.5 + 0.9*1.0 + 0.5*2.0 + 0.8*0.5 + 1.2*1.5 + 0.7*1.0 + 0.6*2.0
    // = 5.0 + 0.4 + 0.9 + 1.0 + 0.4 + 1.8 + 0.7 + 1.2 = 11.4
    assert!((score - 11.4).abs() < 1e-9);
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

    let mock = MockRetriever {
        results: candidates,
    };
    let weights = RankingWeights {
        lexical: 1.0,
        semantic: 2.0,
        recency: 0.0,
        importance: 0.0,
        provenance_confidence: 0.0,
        graph_degree: 0.0,
        access_frequency: 0.0,
        freshness_decay: 0.0,
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
fn test_feature_extractor_math() {
    let decay = RankingDecay {
        recency_half_life_days: 7.0,
        freshness_half_life_days: 1.0,
    };
    let reference_time = 1_000_000u64;
    let extractor = FeatureExtractor::new(reference_time, decay);

    let result = RetrievalResult {
        node_id: NodeId(uuid::Uuid::new_v4()),
        channel_scores: HashMap::from([
            (RetrievalChannel::Fts, 5.0),
            (RetrievalChannel::Semantic, 0.5),
        ]),
        ranking_score: None,
    };

    let context = FeatureContext {
        updated_at: Some(reference_time - 86400 * 7), // 7 days ago
        importance: Some(0.6),
        pinned: true, // Should force importance to 1.0
        provenance_confidence: Some(0.85),
        graph_degree: Some(3), // log(4.0) = 1.386294361
        access_count: Some(1), // log(2.0) = 0.69314718
        last_observed_at: Some(reference_time - 86400), // 1 day ago
    };

    let features = extractor.extract(&result, &context);

    assert_eq!(features.lexical_similarity, Some(5.0));
    assert_eq!(features.semantic_similarity, Some(0.5));

    // Recency decay: dt = 7 days. Expected = 0.5
    assert!((features.recency.unwrap() - 0.5).abs() < 1e-9);

    // Importance: should be 1.0 because pinned is true
    assert_eq!(features.importance, Some(1.0));

    assert_eq!(features.provenance_confidence, Some(0.85));

    // Graph degree: ln(4.0)
    assert!((features.graph_degree.unwrap() - 4.0f64.ln()).abs() < 1e-9);

    // Access frequency: ln(2.0)
    assert!((features.access_frequency.unwrap() - 2.0f64.ln()).abs() < 1e-9);

    // Freshness decay: dt = 1 day. Expected = 0.5
    assert!((features.freshness_decay.unwrap() - 0.5).abs() < 1e-9);
}

#[test]
fn test_feature_provider_loading() {
    let test_storage = TestStorage::new();
    let sqlite = test_storage.storage();

    let node_id_1 = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let node_id_2 = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap());

    // 1. Create nodes with specific metadata
    let mut node1 = Node::new(node_id_1, "rust".to_string(), NodeType::Concept);
    node1.updated_at = 500;
    node1.provenance.confidence = 0.75;
    node1
        .properties
        .insert("importance".to_string(), serde_json::json!(0.8));
    node1
        .properties
        .insert("pinned".to_string(), serde_json::json!(true));
    node1
        .properties
        .insert("provenance_confidence".to_string(), serde_json::json!(0.75));

    let mut node2 = Node::new(node_id_2, "python".to_string(), NodeType::Concept);
    node2.updated_at = 800;
    node2.provenance.confidence = 0.95;
    node2
        .properties
        .insert("provenance_confidence".to_string(), serde_json::json!(0.95));

    sqlite.nodes().save(&node1).unwrap();
    sqlite.nodes().save(&node2).unwrap();

    // 2. Add temporal edge to test graph_degree and last_observed_at
    sqlite
        .save_temporal_edge(&brain_domain::TemporalEdge {
            edge: Edge::new(node_id_1, node_id_2, RelationKind::Uses, 1.0),
            observed_at: brain_domain::TimePoint::from_unix_seconds(900),
            validity: brain_domain::temporal::TemporalValidity::new(vec![]),
        })
        .unwrap();

    // 3. Add feedback event to test access_count
    let conn = sqlite.pool().get().unwrap();
    conn.execute(
        "INSERT INTO feedback_events (id, schema_version, query, node_id, selected, timestamp, ranking_position, context) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        brain_storage::rusqlite::params![
            "event_1",
            1,
            "rust query",
            node_id_1.0.to_string(),
            1,
            950,
            0,
            "{}"
        ],
    ).unwrap();

    // 4. Retrieve batch metadata
    let provider = FeatureProvider::new(sqlite.pool().clone());
    let contexts = provider.load_contexts(&[node_id_1, node_id_2]).unwrap();

    assert_eq!(contexts.len(), 2);

    let ctx1 = contexts.get(&node_id_1).unwrap();
    assert_eq!(ctx1.updated_at, Some(500));
    assert_eq!(ctx1.provenance_confidence, Some(0.75));
    assert_eq!(ctx1.importance, Some(0.8));
    assert!(ctx1.pinned);
    assert_eq!(ctx1.graph_degree, Some(1));
    assert_eq!(ctx1.access_count, Some(1));
    assert_eq!(ctx1.last_observed_at, Some(900));

    let ctx2 = contexts.get(&node_id_2).unwrap();
    assert_eq!(ctx2.updated_at, Some(800));
    assert_eq!(ctx2.provenance_confidence, Some(0.95));
    assert_eq!(ctx2.importance, None);
    assert!(!ctx2.pinned);
    assert_eq!(ctx2.graph_degree, Some(1));
    assert_eq!(ctx2.access_count, Some(0));
    assert_eq!(ctx2.last_observed_at, Some(900));
}

#[test]
fn test_ranking_retriever_harness_integration() {
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

    let ground_truth_json = format!(
        r#"{{
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
                "acceptable_alternatives": ["{}"],
                "minimum_rank": {{}}
            }}
        }}
    }}"#,
        node_b_str, node_a_str, node_b_str, node_a_str
    );

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

    let mock = MockRetriever {
        results: candidates,
    };
    let weights = RankingWeights {
        lexical: 0.1,
        semantic: 10.0,
        recency: 0.0,
        importance: 0.0,
        provenance_confidence: 0.0,
        graph_degree: 0.0,
        access_frequency: 0.0,
        freshness_decay: 0.0,
    };
    let ranker = LinearRanker::new(weights);
    let ranking_retriever = RankingRetriever::new(mock, ranker);

    let report =
        run_benchmark(queries_json, &ground_truth_json, &ranking_retriever, "cold").unwrap();

    assert_eq!(report.stable.metrics.total_queries, 1);
    assert_eq!(report.stable.metrics.successful_queries, 1);
    assert!((report.stable.metrics.mean_ndcg_at_5 - 1.0).abs() < 1e-9);

    let q_diag = &report.measured.diagnostics[0];
    assert_eq!(q_diag.candidates.len(), 2);

    let cand_b = q_diag
        .candidates
        .iter()
        .find(|c| c.node_id == node_b_str)
        .unwrap();
    assert_eq!(cand_b.ranked_score, Some(9.8));
}
