use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use brain_core::RepositorySet;
use brain_domain::{Edge, Node, NodeId, NodeType, RelationKind};
use brain_services::eval_harness::{
    CalibrationEngine, CalibrationObjective, CalibrationOptions, EvaluationSession, FtsRetriever,
    GroundTruthCorpus, HybridRetriever, MarkdownReportWriter, QueryCorpus, RankingDecay,
    SemanticRetriever, FeatureProvider,
};
use brain_storage::TestStorage;
use std::fs;
use std::path::Path;
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
fn test_calibration_grid_search_and_report_generation() {
    // 1. Initialize temporary test storage (runs migrations)
    let test_storage = TestStorage::new();
    let sqlite = test_storage.storage();

    // 2. Load ground truth and query corpus
    let queries_json = include_str!("evaluation/queries.json");
    let ground_truth_json = include_str!("evaluation/ground_truth.json");
    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();
    let queries: QueryCorpus = serde_json::from_str(queries_json).unwrap();

    // 3. Populate SQLite database with nodes
    for corpus_node in &ground_truth.nodes {
        let node_id = NodeId(uuid::Uuid::parse_str(&corpus_node.node_id).unwrap());
        let mut node = Node::new(node_id, corpus_node.content.clone(), NodeType::Concept);
        node.updated_at = 1000000;
        node.properties.insert("importance".to_string(), serde_json::json!(0.5));
        node.properties.insert("provenance_confidence".to_string(), serde_json::json!(0.9));
        sqlite.nodes().save(&node).unwrap();

        // Save a mock embedding (dimension 3) for semantic similarity
        sqlite
            .embeddings()
            .save(&brain_domain::Embedding::new(node_id, vec![1.0, 0.0, 0.0]))
            .unwrap();
    }

    // 4. Save a few temporal edges to populate graph degree
    if ground_truth.nodes.len() >= 2 {
        let node_id_1 = NodeId(uuid::Uuid::parse_str(&ground_truth.nodes[0].node_id).unwrap());
        let node_id_2 = NodeId(uuid::Uuid::parse_str(&ground_truth.nodes[1].node_id).unwrap());
        sqlite
            .save_temporal_edge(&brain_domain::TemporalEdge {
                edge: Edge::new(node_id_1, node_id_2, RelationKind::Uses, 1.0),
                observed_at: brain_domain::TimePoint::from_unix_seconds(950000),
                validity: brain_domain::temporal::TemporalValidity::new(vec![]),
            })
            .unwrap();
    }

    // 5. Add a feedback event to test access counts
    if !ground_truth.nodes.is_empty() {
        let node_id_1 = NodeId(uuid::Uuid::parse_str(&ground_truth.nodes[0].node_id).unwrap());
        let conn = sqlite.pool().get().unwrap();
        conn.execute(
            "INSERT INTO feedback_events (id, schema_version, query, node_id, selected, timestamp, ranking_position, context) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "event_1",
                1,
                "rust query",
                node_id_1.0.to_string(),
                1,
                960000,
                0,
                "{}"
            ],
        ).unwrap();
    }

    // 6. Instantiate Retrievers
    let fts = FtsRetriever::new(sqlite.pool().clone());
    let embed_provider = Arc::new(MockEmbeddingProvider {
        vector: vec![1.0, 0.0, 0.0],
    });
    let semantic = SemanticRetriever::new(sqlite.pool().clone(), embed_provider);
    let hybrid = HybridRetriever::new(fts, semantic);
    let provider = FeatureProvider::new(sqlite.pool().clone());

    let decay = RankingDecay {
        recency_half_life_days: 7.0,
        freshness_half_life_days: 1.0,
    };

    // 7. Build EvaluationSession (Executes candidate generation and provider context lookup ONCE)
    let session = EvaluationSession::build(
        &queries,
        &ground_truth,
        &hybrid,
        &provider,
        1000000, // reference_time
        decay,
    )
    .unwrap();

    // 8. Run Calibration Engine over Grid Options
    let options = CalibrationOptions::Grid {
        lexical_weights: vec![1.0],
        semantic_weights: vec![0.0, 0.5, 1.0, 2.0],
        recency_weights: vec![0.0, 0.5, 1.0],
        importance_weights: vec![0.0, 1.0],
        provenance_weights: vec![0.0],
        graph_degree_weights: vec![0.0],
        access_frequency_weights: vec![0.0],
        freshness_decay_weights: vec![0.0],
    };

    let objective = CalibrationObjective::Composite;
    let results = CalibrationEngine::run_calibration(&session, options, objective);

    assert!(!results.is_empty(), "Calibration should produce at least one result");

    // 9. Generate Report using MarkdownReportWriter
    let report_content = MarkdownReportWriter::write_report(&results, objective);

    // Save report to tests/evaluation/calibration_report.md
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/evaluation");
    fs::create_dir_all(&base_path).unwrap();
    fs::write(base_path.join("calibration_report.md"), &report_content).unwrap();

    // Verify ordering by objective score
    for i in 0..(results.len() - 1) {
        let score_a = objective.score(&results[i]);
        let score_b = objective.score(&results[i + 1]);
        assert!(
            score_a >= score_b,
            "Results must be sorted descending by objective score. Rank {}: {}, Rank {}: {}",
            i + 1,
            score_a,
            i + 2,
            score_b
        );
    }
}
