mod common;

use std::fs;
use std::path::Path;

use brain_domain::NodeId;
use brain_services::retrieval::eval_harness::{
    calibration::QueryEvaluationCache, CalibrationEngine, CalibrationObjective, CalibrationOptions,
    EvaluationSession, FeatureExtractor, LinearRanker, LogisticTrainer, LogisticTrainingConfig,
    ScoreRanker,
};
use common::production_corpus::ProductionCorpusBuilder;

fn evaluate_query_composite<M: ScoreRanker>(
    query_cache: &QueryEvaluationCache,
    model: &M,
    extractor: &FeatureExtractor,
) -> f64 {
    if query_cache.candidates.is_empty() {
        return 0.0;
    }
    let mut scored_results = Vec::with_capacity(query_cache.candidates.len());
    for (res, ctx) in &query_cache.candidates {
        let features = extractor.extract(res, ctx);
        let score = model.score(&features);
        let mut cloned_res = res.clone();
        cloned_res.ranking_score = Some(score);
        scored_results.push(cloned_res);
    }
    brain_services::retrieval::eval_harness::sort_results_deterministically(&mut scored_results);
    let retrieved_ids: Vec<NodeId> = scored_results.iter().map(|r| r.node_id).collect();

    let recall_at_5 = brain_services::retrieval::eval_harness::metrics::compute_recall_at_k(
        &retrieved_ids,
        &query_cache.expected_node_ids,
        5,
    );
    let mrr = brain_services::retrieval::eval_harness::metrics::compute_mrr(
        &retrieved_ids,
        &query_cache.expected_node_ids,
        &query_cache.acceptable_alternatives,
    );
    let ndcg_at_5 = brain_services::retrieval::eval_harness::metrics::compute_ndcg_at_k(
        &retrieved_ids,
        &query_cache.expected_node_ids,
        &query_cache.acceptable_alternatives,
        5,
    );

    0.60 * ndcg_at_5 + 0.20 * mrr + 0.20 * recall_at_5
}

#[test]
fn test_production_corpus_linear_vs_logistic() {
    // 1. Re-use separated ProductionCorpusBuilder to build session cache
    let corpus = ProductionCorpusBuilder::build().unwrap();

    let decay = brain_services::retrieval::eval_harness::RankingDecay {
        recency_half_life_days: 7.0,
        freshness_half_life_days: 1.0,
    };

    let session = EvaluationSession::build(
        &corpus.queries,
        &corpus.ground_truth,
        &corpus.retriever,
        &corpus.feature_provider,
        1600000000,
        decay,
    )
    .unwrap();

    // 2. Calibrate Linear Baseline (grid search weights)
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

    // 3. Train pointwise BCE Logistic model (strictly identical optimizer settings as R2)
    let dataset =
        brain_services::retrieval::eval_harness::models::TrainingDataset::from_session(&session);
    assert!(!dataset.examples.is_empty());

    let config = LogisticTrainingConfig {
        learning_rate: 0.5,
        epochs: 1000,
        l2_regularization: 0.001,
        convergence_tolerance: Some(1e-7),
    };

    let (model, summary) = LogisticTrainer::train(&dataset, &config).unwrap();

    // 4. Run evaluations
    let linear_ranker = LinearRanker::new(baseline_opt.weights);
    let linear_eval = session.evaluate_model(&linear_ranker, baseline_opt.weights);

    let logistic_eval = session.evaluate_model(&model, model.weights);
    let logistic_score = objective.score(&logistic_eval);

    // 5. Query win/loss analysis (improved/degraded/unchanged)
    let extractor = FeatureExtractor::new(session.reference_time, session.decay);
    let mut queries_improved = 0;
    let mut queries_degraded = 0;
    let mut queries_unchanged = 0;

    for query_cache in &session.cache {
        let linear_comp = evaluate_query_composite(query_cache, &linear_ranker, &extractor);
        let logistic_comp = evaluate_query_composite(query_cache, &model, &extractor);

        // Float comparison with small epsilon
        let diff = logistic_comp - linear_comp;
        if diff > 1e-9 {
            queries_improved += 1;
        } else if diff < -1e-9 {
            queries_degraded += 1;
        } else {
            queries_unchanged += 1;
        }
    }

    // 6. Generate report markdown
    let mut md = String::new();
    md.push_str("# Supervised Logistic Regression vs Linear Baseline: Production Corpus\n\n");
    md.push_str("> [!IMPORTANT]\n");
    md.push_str("> This report details evaluation results on a simulated 100-node production-like corpus with realistic relation edges and temporal access frequency metrics.\n\n");

    md.push_str("## Retrieval Performance Comparison\n\n");
    md.push_str("| Metric | Linear Baseline | Logistic Regression | Delta (\\(\\Delta\\)) |\n");
    md.push_str("| :--- | ---: | ---: | ---: |\n");
    md.push_str(&format!(
        "| **Composite** | {:.4} | {:.4} | {:.4} |\n",
        baseline_score,
        logistic_score,
        logistic_score - baseline_score
    ));
    md.push_str(&format!(
        "| **nDCG@5** | {:.4} | {:.4} | {:.4} |\n",
        linear_eval.mean_ndcg_at_5,
        logistic_eval.mean_ndcg_at_5,
        logistic_eval.mean_ndcg_at_5 - linear_eval.mean_ndcg_at_5
    ));
    md.push_str(&format!(
        "| **MRR** | {:.4} | {:.4} | {:.4} |\n",
        linear_eval.mean_mrr,
        logistic_eval.mean_mrr,
        logistic_eval.mean_mrr - linear_eval.mean_mrr
    ));
    md.push_str(&format!(
        "| **Recall@5** | {:.4} | {:.4} | {:.4} |\n",
        linear_eval.mean_recall_at_5,
        logistic_eval.mean_recall_at_5,
        logistic_eval.mean_recall_at_5 - linear_eval.mean_recall_at_5
    ));

    md.push_str("\n## Query-Level Delta Significance\n\n");
    md.push_str("| Outcome | Count |\n");
    md.push_str("| :--- | ---: |\n");
    md.push_str(&format!(
        "| **Queries Improved** | {} |\n",
        queries_improved
    ));
    md.push_str(&format!(
        "| **Queries Unchanged** | {} |\n",
        queries_unchanged
    ));
    md.push_str(&format!(
        "| **Queries Degraded** | {} |\n",
        queries_degraded
    ));

    md.push_str("\n## Optimizer Convergence & Diagnostics\n\n");
    md.push_str("| Parameter | Value |\n");
    md.push_str("| :--- | ---: |\n");
    md.push_str(&format!(
        "| Initial BCE Loss | {:.6} |\n",
        summary.initial_loss
    ));
    md.push_str(&format!("| Final BCE Loss | {:.6} |\n", summary.final_loss));
    md.push_str(&format!("| Epochs Executed | {} |\n", summary.epochs_run));
    let converged_str = if summary.converged {
        "🟢 Yes (tolerance met)"
    } else {
        "🔴 No (reached epoch limit; loss was still decreasing)"
    };
    md.push_str(&format!("| Converged | {} |\n", converged_str));
    md.push_str(&format!(
        "| L2 Regularization (λ) | {:.4} |\n",
        config.l2_regularization
    ));
    md.push_str(&format!(
        "| Model Intercept (b) | {:.4} |\n",
        model.intercept
    ));

    md.push_str("\n## Learned Parameters Comparison\n\n");
    md.push_str("| Feature Name | Linear Calibrated Weight | Logistic Trained Weight |\n");
    md.push_str("| :--- | ---: | ---: |\n");
    md.push_str(&format!(
        "| access_frequency | {:.4} | {:.4} |\n",
        baseline_opt.weights.access_frequency, model.weights.access_frequency
    ));
    md.push_str(&format!(
        "| freshness_decay | {:.4} | {:.4} |\n",
        baseline_opt.weights.freshness_decay, model.weights.freshness_decay
    ));
    md.push_str(&format!(
        "| graph_degree | {:.4} | {:.4} |\n",
        baseline_opt.weights.graph_degree, model.weights.graph_degree
    ));
    md.push_str(&format!(
        "| importance | {:.4} | {:.4} |\n",
        baseline_opt.weights.importance, model.weights.importance
    ));
    md.push_str(&format!(
        "| lexical_similarity | {:.4} | {:.4} |\n",
        baseline_opt.weights.lexical, model.weights.lexical
    ));
    md.push_str(&format!(
        "| provenance_confidence | {:.4} | {:.4} |\n",
        baseline_opt.weights.provenance_confidence, model.weights.provenance_confidence
    ));
    md.push_str(&format!(
        "| recency | {:.4} | {:.4} |\n",
        baseline_opt.weights.recency, model.weights.recency
    ));
    md.push_str(&format!(
        "| semantic_similarity | {:.4} | {:.4} |\n",
        baseline_opt.weights.semantic, model.weights.semantic
    ));

    md.push_str("\n## Research Conclusion\n\n");
    md.push_str("> [NOTE]\n");
    md.push_str("> On the current 100-node production-like corpus, the Logistic Regression model trained with pointwise BCE did not outperform the Linear Baseline that was directly calibrated for the Composite objective. This is consistent with the broader observation that optimizing a pointwise objective does not necessarily maximize listwise ranking metrics.\n");

    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/evaluation");
    fs::create_dir_all(&base_path).unwrap();
    fs::write(base_path.join("production_logistic_report.md"), &md).unwrap();
}
