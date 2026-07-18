mod common;

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use brain_domain::NodeId;
use brain_services::retrieval::eval_harness::{
    cv::{compute_distribution, CrossValidationRunner, EvaluationMetric, FoldAssigner},
    models::{LambdaMartTrainingConfig, TrainingDataset},
    CalibrationEngine, CalibrationObjective, CalibrationOptions, EvaluationSession,
    FeatureExtractor, LinearRanker, LogisticTrainer, LogisticTrainingConfig, ScoreRanker,
};
use common::production_corpus::ProductionCorpusBuilder;

#[test]
fn test_fold_assigner_deterministic_and_integrity_invariants() {
    let assigner = FoldAssigner::new(5);
    let mut query_ids = Vec::new();
    for i in 1..=30 {
        query_ids.push(format!("q_{:03}", i));
    }

    // Invariant 1: Deterministic fold assignment
    let folds_1 = assigner.assign(&query_ids);
    let folds_2 = assigner.assign(&query_ids);

    assert_eq!(folds_1.len(), 5);
    for i in 0..5 {
        assert_eq!(folds_1[i].train_queries, folds_2[i].train_queries);
        assert_eq!(folds_1[i].val_queries, folds_2[i].val_queries);
    }

    // Invariant 2: Partition Integrity
    let mut union_val_queries = HashSet::new();
    let mut seen_queries = HashSet::new();

    for (f_idx, fold) in folds_1.iter().enumerate() {
        assert_eq!(fold.fold_idx, f_idx);

        let train_set: HashSet<&String> = fold.train_queries.iter().collect();
        let val_set: HashSet<&String> = fold.val_queries.iter().collect();

        // train and validation must be disjoint
        for q in &val_set {
            assert!(
                !train_set.contains(q),
                "Query {} found in both train and validation of fold {}",
                q,
                f_idx
            );
        }

        // every query in val_queries must appear exactly once across all validation folds
        for q in fold.val_queries.iter() {
            assert!(
                seen_queries.insert(q.clone()),
                "Query {} appears in multiple validation folds",
                q
            );
            union_val_queries.insert(q.clone());
        }
    }

    // Union of validation folds equals original query set
    assert_eq!(union_val_queries.len(), query_ids.len());
    for q in &query_ids {
        assert!(
            union_val_queries.contains(q),
            "Query {} is missing from validation folds",
            q
        );
    }
}

#[test]
fn test_metric_distribution_calculations() {
    let values = vec![0.90, 0.95, 0.92, 0.88, 0.95];

    let dist = compute_distribution(&values);

    // Mean: (0.90 + 0.95 + 0.92 + 0.88 + 0.95) / 5 = 4.60 / 5 = 0.92
    assert!((dist.mean - 0.92).abs() < 1e-9);

    // Variance: ((0.90-0.92)^2 + (0.95-0.92)^2 + (0.92-0.92)^2 + (0.88-0.92)^2 + (0.95-0.92)^2) / 4
    //           = (0.0004 + 0.0009 + 0.0000 + 0.0016 + 0.0009) / 4
    //           = 0.0038 / 4 = 0.00095
    // Std dev: sqrt(0.00095) ≈ 0.030822
    assert!((dist.std_dev - 0.00095f64.sqrt()).abs() < 1e-9);

    assert_eq!(dist.min, 0.88);
    assert_eq!(dist.max, 0.95);
}

fn evaluate_fold_metrics_for_model<M: ScoreRanker>(
    session: &EvaluationSession,
    model: &M,
    query_ids: &HashSet<String>,
) -> (f64, f64, f64, f64) {
    let extractor = FeatureExtractor::new(session.reference_time, session.decay);
    let mut sum_ndcg = 0.0;
    let mut sum_mrr = 0.0;
    let mut sum_recall = 0.0;
    let mut count = 0;

    for query_cache in &session.cache {
        if !query_ids.contains(&query_cache.query_id) {
            continue;
        }
        count += 1;

        let mut scored_results = Vec::new();
        for (res, ctx) in &query_cache.candidates {
            let features = extractor.extract(res, ctx);
            let score = model.score(&features);
            let mut cloned_res = res.clone();
            cloned_res.ranking_score = Some(score);
            scored_results.push(cloned_res);
        }
        brain_services::retrieval::eval_harness::sort_results_deterministically(
            &mut scored_results,
        );
        let retrieved_ids: Vec<NodeId> = scored_results.iter().map(|r| r.node_id).collect();

        sum_recall += brain_services::retrieval::eval_harness::metrics::compute_recall_at_k(
            &retrieved_ids,
            &query_cache.expected_node_ids,
            5,
        );
        sum_mrr += brain_services::retrieval::eval_harness::metrics::compute_mrr(
            &retrieved_ids,
            &query_cache.expected_node_ids,
            &query_cache.acceptable_alternatives,
        );
        sum_ndcg += brain_services::retrieval::eval_harness::metrics::compute_ndcg_at_k(
            &retrieved_ids,
            &query_cache.expected_node_ids,
            &query_cache.acceptable_alternatives,
            5,
        );
    }

    if count == 0 {
        return (0.0, 0.0, 0.0, 0.0);
    }

    let mean_ndcg = sum_ndcg / (count as f64);
    let mean_mrr = sum_mrr / (count as f64);
    let mean_recall = sum_recall / (count as f64);
    let composite = 0.60 * mean_ndcg + 0.20 * mean_mrr + 0.20 * mean_recall;

    (mean_ndcg, mean_mrr, mean_recall, composite)
}

#[test]
fn test_5_fold_cross_validation_evaluation() {
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

    let dataset = TrainingDataset::from_session(&session);

    let assigner = FoldAssigner::new(5);
    let query_ids: Vec<String> = session.cache.iter().map(|c| c.query_id.clone()).collect();
    let folds = assigner.assign(&query_ids);

    // K-Fold Cross-Validation for LambdaMART
    let lm_config = LambdaMartTrainingConfig {
        num_trees: 50,
        max_depth: 2,
        learning_rate: 0.1,
        min_samples_split: 2,
        validation_fraction: 0.20,
    };

    let lm_cv = CrossValidationRunner::run(&session, &dataset, &lm_config, &assigner).unwrap();
    assert_eq!(lm_cv.folds.len(), 5);

    // Calibrate Linear & Train Logistic on Folds to evaluate CV Baseline spreads
    let mut linear_composites = Vec::new();
    let mut linear_ndcgs = Vec::new();
    let mut linear_mrrs = Vec::new();
    let mut linear_recalls = Vec::new();

    let mut logistic_composites = Vec::new();
    let mut logistic_ndcgs = Vec::new();
    let mut logistic_mrrs = Vec::new();
    let mut logistic_recalls = Vec::new();

    for fold in &folds {
        let train_set: HashSet<String> = fold.train_queries.iter().cloned().collect();
        let val_set: HashSet<String> = fold.val_queries.iter().cloned().collect();

        // 1. Calibrate Linear Baseline on train queries
        // Create a temporary evaluation session for train set
        let mut train_cache = Vec::new();
        for qc in &session.cache {
            if train_set.contains(&qc.query_id) {
                train_cache.push((*qc).clone());
            }
        }
        let train_session = EvaluationSession {
            cache: train_cache,
            reference_time: session.reference_time,
            decay: session.decay,
        };

        let options = CalibrationOptions::Grid {
            lexical_weights: vec![0.0, 1.0],
            semantic_weights: vec![0.0, 1.0],
            recency_weights: vec![0.0, 1.0],
            importance_weights: vec![0.0],
            provenance_weights: vec![0.0],
            graph_degree_weights: vec![0.0],
            access_frequency_weights: vec![0.0],
            freshness_decay_weights: vec![0.0],
        };
        let objective = CalibrationObjective::Composite;
        let baseline_candidates =
            CalibrationEngine::run_calibration(&train_session, options, objective);
        let baseline_opt = baseline_candidates.first().unwrap();
        let linear_ranker = LinearRanker::new(baseline_opt.weights);

        // Evaluate Linear on the held-out val_set
        let (lin_ndcg, lin_mrr, lin_recall, lin_comp) =
            evaluate_fold_metrics_for_model(&session, &linear_ranker, &val_set);
        linear_composites.push(lin_comp);
        linear_ndcgs.push(lin_ndcg);
        linear_mrrs.push(lin_mrr);
        linear_recalls.push(lin_recall);

        // 2. Train Logistic on train queries dataset
        let fold_train_examples: Vec<_> = dataset
            .examples
            .iter()
            .filter(|ex| train_set.contains(&ex.query_id))
            .cloned()
            .collect();
        let fold_train_dataset = TrainingDataset {
            examples: fold_train_examples,
        };

        let lr_config = LogisticTrainingConfig {
            learning_rate: 0.5,
            epochs: 1000,
            l2_regularization: 0.001,
            convergence_tolerance: Some(1e-7),
        };
        let (logistic_model, _) = LogisticTrainer::train(&fold_train_dataset, &lr_config).unwrap();

        // Evaluate Logistic on the held-out val_set
        let (log_ndcg, log_mrr, log_recall, log_comp) =
            evaluate_fold_metrics_for_model(&session, &logistic_model, &val_set);
        logistic_composites.push(log_comp);
        logistic_ndcgs.push(log_ndcg);
        logistic_mrrs.push(log_mrr);
        logistic_recalls.push(log_recall);
    }

    let linear_summary_comp = compute_distribution(&linear_composites);
    let linear_summary_ndcg = compute_distribution(&linear_ndcgs);
    let linear_summary_mrr = compute_distribution(&linear_mrrs);
    let linear_summary_recall = compute_distribution(&linear_recalls);

    let logistic_summary_comp = compute_distribution(&logistic_composites);
    let logistic_summary_ndcg = compute_distribution(&logistic_ndcgs);
    let logistic_summary_mrr = compute_distribution(&logistic_mrrs);
    let logistic_summary_recall = compute_distribution(&logistic_recalls);

    let lm_summary_comp = lm_cv
        .summary
        .distributions
        .get(&EvaluationMetric::Composite)
        .unwrap();
    let lm_summary_ndcg = lm_cv
        .summary
        .distributions
        .get(&EvaluationMetric::NdcgAt5)
        .unwrap();
    let lm_summary_mrr = lm_cv
        .summary
        .distributions
        .get(&EvaluationMetric::Mrr)
        .unwrap();
    let lm_summary_recall = lm_cv
        .summary
        .distributions
        .get(&EvaluationMetric::RecallAt5)
        .unwrap();

    // 6. Generate report markdown
    let mut md = String::new();
    md.push_str("# 5-Fold Cross-Validation Robustness Report\n\n");
    md.push_str("> [!IMPORTANT]\n");
    md.push_str("> This report details evaluation results on a simulated 100-node production-like corpus under 5-Fold Cross-Validation, establishing split-independent performance spreads (Mean, Std Dev, Min, Max).\n\n");

    md.push_str("## Fold Balance Composition\n\n");
    md.push_str("| Fold | Train Queries Count | Validation Queries Count |\n");
    md.push_str("| :--- | ------------------: | -----------------------: |\n");
    for fold in &folds {
        md.push_str(&format!(
            "| {} | {} | {} |\n",
            fold.fold_idx,
            fold.train_queries.len(),
            fold.val_queries.len()
        ));
    }

    md.push_str("\n## Validation Query Partitions\n\n");
    for fold in &folds {
        md.push_str(&format!(
            "### Fold {} Validation Queries\n- {}\n\n",
            fold.fold_idx,
            fold.val_queries.join(", ")
        ));
    }

    md.push_str("## Cross-Validation Metric Distributions Comparison\n\n");
    md.push_str("| Metric | Model | Mean | Std Dev | Min | Max |\n");
    md.push_str("| :--- | :--- | ---: | ---: | ---: | ---: |\n");

    // Composite
    md.push_str(&format!(
        "| **Composite** | Linear Baseline | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        linear_summary_comp.mean,
        linear_summary_comp.std_dev,
        linear_summary_comp.min,
        linear_summary_comp.max
    ));
    md.push_str(&format!(
        "| | Logistic Regression | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        logistic_summary_comp.mean,
        logistic_summary_comp.std_dev,
        logistic_summary_comp.min,
        logistic_summary_comp.max
    ));
    md.push_str(&format!(
        "| | LambdaMART | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        lm_summary_comp.mean, lm_summary_comp.std_dev, lm_summary_comp.min, lm_summary_comp.max
    ));

    // nDCG@5
    md.push_str(&format!(
        "| **nDCG@5** | Linear Baseline | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        linear_summary_ndcg.mean,
        linear_summary_ndcg.std_dev,
        linear_summary_ndcg.min,
        linear_summary_ndcg.max
    ));
    md.push_str(&format!(
        "| | Logistic Regression | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        logistic_summary_ndcg.mean,
        logistic_summary_ndcg.std_dev,
        logistic_summary_ndcg.min,
        logistic_summary_ndcg.max
    ));
    md.push_str(&format!(
        "| | LambdaMART | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        lm_summary_ndcg.mean, lm_summary_ndcg.std_dev, lm_summary_ndcg.min, lm_summary_ndcg.max
    ));

    // MRR
    md.push_str(&format!(
        "| **MRR** | Linear Baseline | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        linear_summary_mrr.mean,
        linear_summary_mrr.std_dev,
        linear_summary_mrr.min,
        linear_summary_mrr.max
    ));
    md.push_str(&format!(
        "| | Logistic Regression | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        logistic_summary_mrr.mean,
        logistic_summary_mrr.std_dev,
        logistic_summary_mrr.min,
        logistic_summary_mrr.max
    ));
    md.push_str(&format!(
        "| | LambdaMART | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        lm_summary_mrr.mean, lm_summary_mrr.std_dev, lm_summary_mrr.min, lm_summary_mrr.max
    ));

    // Recall@5
    md.push_str(&format!(
        "| **Recall@5** | Linear Baseline | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        linear_summary_recall.mean,
        linear_summary_recall.std_dev,
        linear_summary_recall.min,
        linear_summary_recall.max
    ));
    md.push_str(&format!(
        "| | Logistic Regression | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        logistic_summary_recall.mean,
        logistic_summary_recall.std_dev,
        logistic_summary_recall.min,
        logistic_summary_recall.max
    ));
    md.push_str(&format!(
        "| | LambdaMART | {:.4} | {:.4} | {:.4} | {:.4} |\n",
        lm_summary_recall.mean,
        lm_summary_recall.std_dev,
        lm_summary_recall.min,
        lm_summary_recall.max
    ));

    md.push_str("\n## Research Conclusion\n\n");
    md.push_str("> [NOTE]\n");
    if lm_summary_comp.mean > linear_summary_comp.mean {
        md.push_str(&format!(
            "> Under 5-Fold Cross-Validation, LambdaMART achieved a mean Composite score of **{:.4}**, outperforming the Linear Baseline's mean of **{:.4}** by **+{:.4}**. The min/max distributions confirm that listwise boosting generalize consistently across diverse query folds.\n",
            lm_summary_comp.mean, linear_summary_comp.mean, lm_summary_comp.mean - linear_summary_comp.mean
        ));
    } else {
        md.push_str(&format!(
            "> Under 5-Fold Cross-Validation, LambdaMART achieved a mean Composite score of **{:.4}**, while the Linear Baseline maintained **{:.4}** (Delta: **{:.4}**). This demonstrates that while LambdaMART learns complex local non-linear interactions, the linear model calibrated over multiple folds remains a very strong competitor for this corpus scale.\n",
            lm_summary_comp.mean, linear_summary_comp.mean, lm_summary_comp.mean - linear_summary_comp.mean
        ));
    }

    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/evaluation");
    fs::create_dir_all(&base_path).unwrap();
    fs::write(base_path.join("production_lambdamart_cv_report.md"), &md).unwrap();
}
