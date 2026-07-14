use brain_core::errors::BrainError;
use brain_domain::NodeId;
use brain_services::eval_harness::{
    run_benchmark, validate_corpus, GroundTruthCorpus, QueryCorpus, RetrievalResult, Retriever,
    RetrievalChannel,
};
use std::collections::HashMap;

struct PerfectRetriever {
    ground_truth: HashMap<String, Vec<NodeId>>,
}

impl PerfectRetriever {
    fn new(truth: &GroundTruthCorpus) -> Self {
        let mut ground_truth = HashMap::new();
        for (q_id, item) in &truth.ground_truth {
            let ids = item
                .expected_node_ids
                .iter()
                .map(|s| NodeId(uuid::Uuid::parse_str(s).unwrap()))
                .collect();
            ground_truth.insert(q_id.clone(), ids);
        }
        Self { ground_truth }
    }
}

impl Retriever for PerfectRetriever {
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        let q_id = match query {
            "How do I configure TypeScript client UDS?" => "q_001",
            "typescript clent socket configuration path" => "q_002",
            "configure TS UDS" => "q_003",
            "How does the Python plugin loader scan files?" => "q_004",
            "loading external scripts via pyo3 modules" => "q_005",
            "parse daemon command line arguments" => "q_006",
            "What happens during background daemon lifecycle?" => "q_007",
            "handling SIGTERM signal in daemon main" => "q_008",
            "DaemonCleanupGuard" => "q_009",
            "removing stale file on start" => "q_010",
            "damon cleanp gard drop" => "q_011",
            "tokio uds connection timeout error" => "q_012",
            "increase connect timeout" => "q_013",
            "blanket implementation macro for capability deserialization" => "q_014",
            "ErasedCapability" => "q_015",
            "how do extensions work" => "q_016",
            "duckdb analytics database initialization error" => "q_017",
            "relational memory engine" => "q_018",
            "recent sqlite graph changes" => "q_019",
            "sqllite ltm databse" => "q_020",
            "type erased registration dynamic mapping" => "q_021",
            "FuzzyRetrieval" => "q_022",
            "graceful worker draining check sleep" => "q_023",
            "how are messages exchanged over UDS IPC" => "q_024",
            _ => return Ok(vec![]),
        };

        if let Some(nodes) = self.ground_truth.get(q_id) {
            Ok(nodes
                .iter()
                .map(|id| RetrievalResult {
                    node_id: *id,
                    channel_scores: std::collections::HashMap::from([(RetrievalChannel::Fts, 1.0)]),
                    ranking_score: None,
                })
                .collect())
        } else {
            Ok(vec![])
        }
    }
}

struct PartialRetriever {
    ground_truth: HashMap<String, Vec<NodeId>>,
}

impl PartialRetriever {
    fn new(truth: &GroundTruthCorpus) -> Self {
        let mut ground_truth = HashMap::new();
        for (q_id, item) in &truth.ground_truth {
            let ids = item
                .expected_node_ids
                .iter()
                .map(|s| NodeId(uuid::Uuid::parse_str(s).unwrap()))
                .collect();
            ground_truth.insert(q_id.clone(), ids);
        }
        Self { ground_truth }
    }
}

impl Retriever for PartialRetriever {
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        let q_id = match query {
            "How do I configure TypeScript client UDS?" => "q_001",
            "typescript clent socket configuration path" => "q_002",
            "configure TS UDS" => "q_003",
            "How does the Python plugin loader scan files?" => "q_004",
            "loading external scripts via pyo3 modules" => "q_005",
            "parse daemon command line arguments" => "q_006",
            "What happens during background daemon lifecycle?" => "q_007",
            "handling SIGTERM signal in daemon main" => "q_008",
            "DaemonCleanupGuard" => "q_009",
            "removing stale file on start" => "q_010",
            "damon cleanp gard drop" => "q_011",
            "tokio uds connection timeout error" => "q_012",
            "increase connect timeout" => "q_013",
            "blanket implementation macro for capability deserialization" => "q_014",
            "ErasedCapability" => "q_015",
            "how do extensions work" => "q_016",
            "duckdb analytics database initialization error" => "q_017",
            "relational memory engine" => "q_018",
            "recent sqlite graph changes" => "q_019",
            "sqllite ltm databse" => "q_020",
            "type erased registration dynamic mapping" => "q_021",
            "FuzzyRetrieval" => "q_022",
            "graceful worker draining check sleep" => "q_023",
            "how are messages exchanged over UDS IPC" => "q_024",
            _ => return Ok(vec![]),
        };

        // For partial matching query q_001, return correct node.
        // For q_004, simulate failure (fails to retrieve).
        // For q_007 (expected: mem_004), return incorrect node (mem_001) instead.
        // For q_008 (expected: mem_004), return expected node but with score 0.1, and incorrect node with score 0.9 (reordering).
        match q_id {
            "q_004" => Err(BrainError::Internal {
                message: "Simulated retrieval error".to_string(),
            }),
            "q_007" => {
                // Return wrong node
                let dummy_id = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
                Ok(vec![RetrievalResult {
                    node_id: dummy_id,
                    channel_scores: std::collections::HashMap::from([(RetrievalChannel::Fts, 1.0)]),
                    ranking_score: None,
                }])
            }
            "q_008" => {
                let expected_id = self.ground_truth.get("q_008").unwrap()[0];
                let dummy_id = NodeId(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
                // Return expected node with lower score, and dummy with higher score (forces reordering)
                Ok(vec![
                    RetrievalResult {
                        node_id: expected_id,
                        channel_scores: std::collections::HashMap::from([(RetrievalChannel::Fts, 0.1)]),
                        ranking_score: None,
                    },
                    RetrievalResult {
                        node_id: dummy_id,
                        channel_scores: std::collections::HashMap::from([(RetrievalChannel::Fts, 0.9)]),
                        ranking_score: None,
                    },
                ])
            }
            _ => {
                if let Some(nodes) = self.ground_truth.get(q_id) {
                    Ok(nodes
                        .iter()
                        .map(|id| RetrievalResult {
                            node_id: *id,
                            channel_scores: std::collections::HashMap::from([(RetrievalChannel::Fts, 1.0)]),
                            ranking_score: None,
                        })
                        .collect())
                } else {
                    Ok(vec![])
                }
            }
        }
    }
}

#[test]
fn test_evaluation_corpus_validation() {
    let queries_json = include_str!("evaluation/queries.json");
    let ground_truth_json = include_str!("evaluation/ground_truth.json");

    let queries: QueryCorpus = serde_json::from_str(queries_json).unwrap();
    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();

    assert!(validate_corpus(&queries, &ground_truth).is_ok());
}

#[test]
fn test_harness_perfect_retriever() {
    let queries_json = include_str!("evaluation/queries.json");
    let ground_truth_json = include_str!("evaluation/ground_truth.json");
    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();

    let retriever = PerfectRetriever::new(&ground_truth);
    let report = run_benchmark(queries_json, ground_truth_json, &retriever, "mock").unwrap();

    assert_eq!(report.stable.metrics.total_queries, 24);
    assert_eq!(report.stable.metrics.successful_queries, 24);
    assert_eq!(report.stable.metrics.failed_queries, 0);

    // Perfect retriever should hit exactly 1.0 on all metrics
    assert_eq!(report.stable.metrics.mean_recall_at_1, 1.0);
    assert_eq!(report.stable.metrics.mean_recall_at_5, 1.0);
    assert_eq!(report.stable.metrics.mean_recall_at_10, 1.0);
    assert_eq!(report.stable.metrics.mean_precision_at_5, 1.0);
    assert_eq!(report.stable.metrics.mean_precision_at_10, 1.0);
    assert_eq!(report.stable.metrics.mean_mrr, 1.0);

    for res in &report.stable.query_results {
        assert_eq!(res.status, "success");
        assert_eq!(res.recall_at_1, 1.0);
        assert_eq!(res.mrr, 1.0);
    }
    for diag in &report.measured.diagnostics {
        assert_eq!(diag.candidates[0].retrieval_channels, vec![RetrievalChannel::Fts]);
    }
}

#[test]
fn test_harness_partial_retriever() {
    let queries_json = include_str!("evaluation/queries.json");
    let ground_truth_json = include_str!("evaluation/ground_truth.json");
    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();

    let retriever = PartialRetriever::new(&ground_truth);
    let report = run_benchmark(queries_json, ground_truth_json, &retriever, "mock").unwrap();

    assert_eq!(report.stable.metrics.total_queries, 24);
    // 23 succeeded, 1 failed (q_004)
    assert_eq!(report.stable.metrics.successful_queries, 23);
    assert_eq!(report.stable.metrics.failed_queries, 1);

    // Check partial error query status
    let q_004_res = report.stable.query_results.iter().find(|r| r.query_id == "q_004").unwrap();
    assert_eq!(q_004_res.status, "retrieval_error");
    assert!(q_004_res.error.is_some());
    assert_eq!(q_004_res.recall_at_1, 0.0);

    // Check wrong node result for q_007
    let q_007_res = report.stable.query_results.iter().find(|r| r.query_id == "q_007").unwrap();
    assert_eq!(q_007_res.status, "success");
    assert_eq!(q_007_res.recall_at_1, 0.0);
    assert_eq!(q_007_res.mrr, 0.0);

    // Check reordering result for q_008
    let q_008_res = report.stable.query_results.iter().find(|r| r.query_id == "q_008").unwrap();
    assert_eq!(q_008_res.status, "success");
    // Expected node was placed second, so recall_at_1 should be 0.0, but recall_at_5 is 1.0. MRR is 0.5.
    assert_eq!(q_008_res.recall_at_1, 0.0);
    assert_eq!(q_008_res.recall_at_5, 1.0);
    assert_eq!(q_008_res.mrr, 0.5);

    // Verify MRR and Recall are less than 1.0 overall
    assert!(report.stable.metrics.mean_recall_at_1 < 1.0);
    assert!(report.stable.metrics.mean_mrr < 1.0);
}

#[test]
fn test_harness_determinism_and_reproducibility() {
    let queries_json = include_str!("evaluation/queries.json");
    let ground_truth_json = include_str!("evaluation/ground_truth.json");
    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();

    let retriever = PartialRetriever::new(&ground_truth);
    let mut report1 = run_benchmark(queries_json, ground_truth_json, &retriever, "mock").unwrap();
    let mut report2 = run_benchmark(queries_json, ground_truth_json, &retriever, "mock").unwrap();

    // Latency is non-deterministic, so zero it out for reproducibility comparison
    report1.measured.latency = Default::default();
    for d in &mut report1.measured.diagnostics {
        d.latency_ms = 0.0;
    }

    report2.measured.latency = Default::default();
    for d in &mut report2.measured.diagnostics {
        d.latency_ms = 0.0;
    }

    let serialized1 = serde_json::to_string(&report1).unwrap();
    let serialized2 = serde_json::to_string(&report2).unwrap();

    // Verify byte-for-byte identical reports (stable & zeroed-latency diagnostics)
    assert_eq!(serialized1, serialized2);
}

#[test]
fn test_harness_invalid_corpus_fails_early() {
    let queries_json = r#"{
        "version": 1,
        "queries": [
            { "query_id": "q_001", "text": "Duplicate query", "tags": [] },
            { "query_id": "q_001", "text": "Duplicate query", "tags": [] }
        ]
    }"#;
    let ground_truth_json = r#"{
        "version": 1,
        "nodes": [],
        "ground_truth": {}
    }"#;

    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();
    let retriever = PerfectRetriever::new(&ground_truth);
    let result = run_benchmark(queries_json, ground_truth_json, &retriever, "mock");

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Duplicate query_id"));
}

#[test]
fn test_retrieval_channel_serialization() {
    assert_eq!(
        serde_json::to_string(&RetrievalChannel::Fts).unwrap(),
        "\"fts\""
    );
    assert_eq!(
        serde_json::to_string(&RetrievalChannel::Semantic).unwrap(),
        "\"semantic\""
    );
    assert_eq!(
        serde_json::to_string(&RetrievalChannel::Metadata).unwrap(),
        "\"metadata\""
    );
}
