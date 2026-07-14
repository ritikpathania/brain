use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use brain_core::RepositorySet;
use brain_domain::{Edge, Node, NodeId, NodeType, RelationKind};
use brain_services::eval_harness::{
    EvaluationSession, FtsRetriever, GroundTruthCorpus, HybridRetriever, QueryCorpus, RankingDecay,
    RankingWeights, SemanticRetriever, FeatureProvider, run_sensitivity_analysis,
    SensitivityReportWriter,
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
fn test_sensitivity_analysis_execution_and_reporting() {
    // 1. Initialize temporary test storage
    let test_storage = TestStorage::new();
    let sqlite = test_storage.storage();

    // 2. Load corpus
    let queries_json = include_str!("evaluation/queries.json");
    let ground_truth_json = include_str!("evaluation/ground_truth.json");
    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();
    let queries: QueryCorpus = serde_json::from_str(queries_json).unwrap();

    // 3. Populate SQLite database with nodes
    for (idx, corpus_node) in ground_truth.nodes.iter().enumerate() {
        let node_id = NodeId(uuid::Uuid::parse_str(&corpus_node.node_id).unwrap());
        let mut node = Node::new(node_id, corpus_node.content.clone(), NodeType::Concept);
        // Vary updated_at to create non-zero variance for recency
        node.updated_at = 1000000 - (idx as u64 * 10000);
        node.properties.insert("importance".to_string(), serde_json::json!(idx as f64 * 0.1));
        sqlite.nodes().save(&node).unwrap();

        // Save a mock embedding
        sqlite
            .embeddings()
            .save(&brain_domain::Embedding::new(node_id, vec![1.0, 0.0, 0.0]))
            .unwrap();
    }

    // 4. Save a temporal edge
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

    // 5. Instantiate retrievers
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

    // 6. Build session
    let session = EvaluationSession::build(
        &queries,
        &ground_truth,
        &hybrid,
        &provider,
        1000000,
        decay,
    )
    .unwrap();

    // 7. Define baseline weights
    let baseline = RankingWeights {
        lexical: 1.0,
        semantic: 1.0,
        recency: 0.5,
        importance: 0.5,
        provenance_confidence: 0.0,
        graph_degree: 0.0,
        access_frequency: 0.0,
        freshness_decay: 0.0,
    };

    // 8. Run Sensitivity Analysis
    let report = run_sensitivity_analysis(&session, baseline);

    assert_eq!(report.impacts.len(), 8);

    // Verify alphabetical ordering of features
    let mut last_name = String::new();
    for imp in &report.impacts {
        assert!(imp.feature_name > last_name, "Features not sorted alphabetically: {} vs {}", imp.feature_name, last_name);
        last_name = imp.feature_name.clone();
    }

    // Verify zero variance flag behavior
    // provenance_confidence should be zero variance because it is not set on nodes (defaults to None / zero std dev)
    let prov = report.impacts.iter().find(|i| i.feature_name == "provenance_confidence").unwrap();
    assert!(prov.zero_variance);

    // 9. Generate Report using SensitivityReportWriter
    let report_content = SensitivityReportWriter::write_report(&report);

    // Save report to tests/evaluation/sensitivity_report.md
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/evaluation");
    fs::create_dir_all(&base_path).unwrap();
    fs::write(base_path.join("sensitivity_report.md"), &report_content).unwrap();
}
