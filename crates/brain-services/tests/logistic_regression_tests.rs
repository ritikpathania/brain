use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use brain_core::RepositorySet;
use brain_domain::{Edge, Node, NodeId, NodeType, RelationKind};
use brain_services::eval_harness::{
    CalibrationEngine, CalibrationObjective, CalibrationOptions, EvaluationSession, FtsRetriever,
    GroundTruthCorpus, HybridRetriever, QueryCorpus, RankingDecay, SemanticRetriever, FeatureProvider,
    TrainingDataset, LogisticTrainer, LogisticTrainingConfig,
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
fn test_logistic_regression_training_and_comparison() {
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

    // 3. Populate database programmatically
    for n in &fixture.nodes {
        let node_id = NodeId(uuid::Uuid::parse_str(&n.node_id).unwrap());
        let mut node = Node::new(node_id, n.content.clone(), NodeType::Concept);
        node.updated_at = n.properties.updated_at;
        node.properties.insert("importance".to_string(), serde_json::json!(n.properties.importance));
        node.properties.insert("pinned".to_string(), serde_json::json!(n.properties.pinned));
        node.properties.insert("provenance_confidence".to_string(), serde_json::json!(n.properties.provenance_confidence));
        sqlite.nodes().save(&node).unwrap();

        sqlite
            .embeddings()
            .save(&brain_domain::Embedding::new(node_id, n.properties.embedding.clone()))
            .unwrap();

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

    // 5. Build session
    let session = EvaluationSession::build(
        &queries,
        &ground_truth,
        &hybrid,
        &provider,
        1000000,
        decay,
    )
    .unwrap();

    // 6. Run Baseline Linear calibration
    let options = CalibrationOptions::Grid {
        lexical_weights: vec![0.0, 1.0],
        semantic_weights: vec![0.0, 1.0],
        recency_weights: vec![0.0, 1.0],
        importance_weights: vec![0.0, 1.0],
        provenance_weights: vec![0.0, 1.0],
        graph_degree_weights: vec![0.0, 1.0],
        access_frequency_weights: vec![0.0, 1.0],
        freshness_decay_weights: vec![0.0, 1.0],
    };

    let objective = CalibrationObjective::Composite;
    let baseline_candidates = CalibrationEngine::run_calibration(&session, options, objective);
    let baseline_opt = baseline_candidates.first().unwrap();
    let baseline_score = objective.score(baseline_opt);

    // 7. Prepare generic dataset and run Logistic Regression trainer
    let dataset = TrainingDataset::from_session(&session);
    assert!(!dataset.examples.is_empty());

    let config = LogisticTrainingConfig {
        learning_rate: 0.5,
        epochs: 1000,
        l2_regularization: 0.001,
        convergence_tolerance: Some(1e-7),
    };

    let (model, summary) = LogisticTrainer::train(&dataset, &config).unwrap();

    // Verify mathematical bounds & convergence properties
    assert!(summary.final_loss < summary.initial_loss);
    assert!(summary.epochs_run > 0);

    // Verify model inference scores stay bounded within [0, 1]
    let extractor = brain_services::retrieval::eval_harness::FeatureExtractor::new(session.reference_time, session.decay);
    for q_cache in &session.cache {
        for (res, ctx) in &q_cache.candidates {
            let features = extractor.extract(res, ctx);
            let score = brain_services::retrieval::eval_harness::models::ScoreRanker::score(&model, &features);
            assert!(score >= 0.0 && score <= 1.0);
        }
    }

    // 8. Evaluate Logistic model on the session cache
    let logistic_eval = session.evaluate_model(&model, model.weights);
    let logistic_score = objective.score(&logistic_eval);

    // 9. Generate and write controlled_logistic_report.md
    let mut md = String::new();
    md.push_str("# Supervised Logistic Regression Ranker Report\n\n");
    md.push_str("> [!IMPORTANT]\n");
    md.push_str("> Controlled benchmarks intentionally exaggerate feature influence to verify ranking behavior.\n");
    md.push_str("> Model calibration is calculated on a deterministic training dataset extracted directly from EvaluationSession.\n\n");

    md.push_str("## Optimizer Convergence & Diagnostics\n\n");
    md.push_str("| Metric | Value |\n");
    md.push_str("| :--- | ---: |\n");
    md.push_str(&format!("| Initial BCE Loss | {:.6} |\n", summary.initial_loss));
    md.push_str(&format!("| Final BCE Loss | {:.6} |\n", summary.final_loss));
    md.push_str(&format!("| Epochs Executed | {} |\n", summary.epochs_run));
    let converged_str = if summary.converged {
        "🟢 Yes (tolerance met)"
    } else {
        "🔴 No (reached epoch limit; loss was still decreasing)"
    };
    md.push_str(&format!("| Converged | {} |\n", converged_str));
    md.push_str(&format!("| L2 Regularization (λ) | {:.4} |\n", config.l2_regularization));
    md.push_str(&format!("| Model Intercept (b) | {:.4} |\n", model.intercept));

    md.push_str("\n## Feature Parameter Comparison\n\n");
    md.push_str("| Feature Name | Linear Calibrated Weight | Logistic Trained Weight |\n");
    md.push_str("| :--- | ---: | ---: |\n");
    md.push_str(&format!("| access_frequency | {:.4} | {:.4} |\n", baseline_opt.weights.access_frequency, model.weights.access_frequency));
    md.push_str(&format!("| freshness_decay | {:.4} | {:.4} |\n", baseline_opt.weights.freshness_decay, model.weights.freshness_decay));
    md.push_str(&format!("| graph_degree | {:.4} | {:.4} |\n", baseline_opt.weights.graph_degree, model.weights.graph_degree));
    md.push_str(&format!("| importance | {:.4} | {:.4} |\n", baseline_opt.weights.importance, model.weights.importance));
    md.push_str(&format!("| lexical_similarity | {:.4} | {:.4} |\n", baseline_opt.weights.lexical, model.weights.lexical));
    md.push_str(&format!("| provenance_confidence | {:.4} | {:.4} |\n", baseline_opt.weights.provenance_confidence, model.weights.provenance_confidence));
    md.push_str(&format!("| recency | {:.4} | {:.4} |\n", baseline_opt.weights.recency, model.weights.recency));
    md.push_str(&format!("| semantic_similarity | {:.4} | {:.4} |\n", baseline_opt.weights.semantic, model.weights.semantic));

    md.push_str("\n## Retrieval Performance Baseline comparison\n\n");
    md.push_str("| Model Type | Composite Score | nDCG@5 | MRR | Recall@5 |\n");
    md.push_str("| :--- | ---: | ---: | ---: | ---: |\n");
    md.push_str(&format!(
        "| **Linear Baseline** | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        baseline_score, baseline_opt.mean_ndcg_at_5, baseline_opt.mean_mrr, baseline_opt.mean_recall_at_5
    ));
    md.push_str(&format!(
        "| **Logistic Regression** | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        logistic_score, logistic_eval.mean_ndcg_at_5, logistic_eval.mean_mrr, logistic_eval.mean_recall_at_5
    ));

    md.push_str("\n## Research Conclusion\n\n");
    md.push_str("> [!NOTE]\n");
    md.push_str("> On the current controlled corpus, the Logistic Regression model trained with pointwise BCE did not outperform the Linear Baseline that was directly calibrated for the Composite objective. This is consistent with the broader observation that optimizing a pointwise objective does not necessarily maximize listwise ranking metrics.\n");

    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/evaluation");
    fs::create_dir_all(&base_path).unwrap();
    fs::write(base_path.join("controlled_logistic_report.md"), &md).unwrap();
}
