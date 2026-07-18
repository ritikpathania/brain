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

struct FixtureQuery {
    _query_id: String,
    embedding: Vec<f32>,
}

struct FixtureEmbeddingProvider {
    text_to_query: std::collections::HashMap<String, FixtureQuery>,
}

impl EmbeddingProvider for FixtureEmbeddingProvider {
    fn name(&self) -> &'static str {
        "fixture-embedding-provider"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let fq = self.text_to_query.get(text).ok_or_else(|| {
            BrainError::Validation { message: format!("Query text not found in fixture: {}", text) }
        })?;
        Ok(fq.embedding.clone())
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FixtureNode {
    node_id: String,
    content: String,
    properties: FixtureProperties,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FixtureProperties {
    embedding: Vec<f32>,
    updated_at: u64,
    pinned: bool,
    importance: f64,
    provenance_confidence: f64,
    access_count: u64,
    last_observed_at: u64,
    edges: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FixtureCorpus {
    nodes: Vec<FixtureNode>,
}

#[test]
fn test_calibration_grid_search_and_report_generation() {
    // 1. Initialize temporary test storage
    let test_storage = TestStorage::new();
    let sqlite = test_storage.storage();

    // 2. Load controlled corpus files
    let queries_json = include_str!("evaluation/controlled_queries.json");
    let ground_truth_json = include_str!("evaluation/controlled_ground_truth.json");
    let fixture_json = include_str!("evaluation/controlled_fixture.json");

    let queries: QueryCorpus = serde_json::from_str(queries_json).unwrap();
    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();
    let fixture: FixtureCorpus = serde_json::from_str(fixture_json).unwrap();

    // 3. Populate database programmatically from fixture metadata (First pass: nodes, embeddings, events)
    for n in &fixture.nodes {
        let node_id = NodeId(uuid::Uuid::parse_str(&n.node_id).unwrap());
        let mut node = Node::new(node_id, n.content.clone(), NodeType::Concept);
        node.updated_at = n.properties.updated_at;
        node.properties.insert("importance".to_string(), serde_json::json!(n.properties.importance));
        node.properties.insert("pinned".to_string(), serde_json::json!(n.properties.pinned));
        node.properties.insert("provenance_confidence".to_string(), serde_json::json!(n.properties.provenance_confidence));
        sqlite.nodes().save(&node).unwrap();

        // Save embedding
        sqlite
            .embeddings()
            .save(&brain_domain::Embedding::new(node_id, n.properties.embedding.clone()))
            .unwrap();

        // Save access feedback events
        if n.properties.access_count > 0 {
            let conn = sqlite.pool().get().unwrap();
            for i in 0..n.properties.access_count {
                conn.execute(
                    "INSERT INTO feedback_events (id, schema_version, query, node_id, selected, timestamp, ranking_position, context) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        format!("event_{}_{}", n.node_id, i),
                        1,
                        "controlled query",
                        n.node_id.clone(),
                        1,
                        n.properties.last_observed_at,
                        0,
                        "{}"
                    ],
                ).unwrap();
            }
        }
    }

    // Second pass: save all edges (guarantees target nodes exist to satisfy foreign keys)
    for n in &fixture.nodes {
        let node_id = NodeId(uuid::Uuid::parse_str(&n.node_id).unwrap());
        for target_str in &n.properties.edges {
            let target_id = NodeId(uuid::Uuid::parse_str(target_str).unwrap());
            sqlite
                .save_temporal_edge(&brain_domain::TemporalEdge {
                    edge: Edge::new(node_id, target_id, RelationKind::Uses, 1.0),
                    observed_at: brain_domain::TimePoint::from_unix_seconds(n.properties.last_observed_at),
                    validity: brain_domain::temporal::TemporalValidity::new(vec![]),
                })
                .unwrap();
        }
    }

    // 4. Instantiate Retrievers
    let fts = FtsRetriever::new(sqlite.pool().clone());
    let mut text_to_query = std::collections::HashMap::new();
    for q in &queries.queries {
        let embedding = q.embedding.clone().ok_or_else(|| {
            BrainError::Validation { message: format!("Embedding not defined for query: {}", q.query_id) }
        }).unwrap();
        text_to_query.insert(
            q.text.clone(),
            FixtureQuery {
                _query_id: q.query_id.clone(),
                embedding,
            },
        );
    }
    let embed_provider = Arc::new(FixtureEmbeddingProvider { text_to_query });
    let semantic = SemanticRetriever::new(sqlite.pool().clone(), embed_provider);
    let hybrid = HybridRetriever::new(fts, semantic);
    let provider = FeatureProvider::new(sqlite.pool().clone());

    let decay = RankingDecay {
        recency_half_life_days: 7.0,
        freshness_half_life_days: 1.0,
    };

    // 5. Build EvaluationSession
    let session = EvaluationSession::build(
        &queries,
        &ground_truth,
        &hybrid,
        &provider,
        1000000, // reference_time
        decay,
    )
    .unwrap();

    // 6. Run Calibration Engine over Grid Options
    let options = CalibrationOptions::Grid {
        lexical_weights: vec![1.0],
        semantic_weights: vec![0.0, 1.0, 5.0],
        recency_weights: vec![0.0, 1.0],
        importance_weights: vec![0.0, 1.0],
        provenance_weights: vec![0.0],
        graph_degree_weights: vec![0.0],
        access_frequency_weights: vec![0.0],
        freshness_decay_weights: vec![0.0],
    };

    let objective = CalibrationObjective::Composite;
    let results = CalibrationEngine::run_calibration(&session, options, objective);

    assert!(!results.is_empty(), "Calibration should produce at least one result");

    // 7. Generate Report using MarkdownReportWriter
    let report_content = MarkdownReportWriter::write_report(&results, objective);

    // Save report to tests/evaluation/controlled_calibration_report.md
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/evaluation");
    fs::create_dir_all(&base_path).unwrap();
    fs::write(base_path.join("controlled_calibration_report.md"), &report_content).unwrap();

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
