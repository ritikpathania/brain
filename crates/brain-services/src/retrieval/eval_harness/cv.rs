use crate::retrieval::eval_harness::models::{
    LambdaMartModel, LambdaMartTrainer, LambdaMartTrainingConfig, ModelSelector, TrainingDataset,
};
use crate::retrieval::eval_harness::{EvaluationSession, FeatureExtractor, NodeId, ScoreRanker};
use brain_core::errors::BrainError;
use std::collections::{BTreeMap, HashSet};

/// Deterministic query fold mapping representing query partition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Fold {
    /// 0-based index of the fold.
    pub fold_idx: usize,
    /// Sorted list of train query IDs.
    pub train_queries: Vec<String>,
    /// Sorted list of validation query IDs.
    pub val_queries: Vec<String>,
}

/// Helper that deterministically round-robins queries into K folds alphabetically.
pub struct FoldAssigner {
    /// Number of folds.
    pub k: usize,
}

impl FoldAssigner {
    /// Instantiates a FoldAssigner with K folds.
    pub fn new(k: usize) -> Self {
        Self { k }
    }

    /// Assigns the query IDs into folds deterministically.
    pub fn assign(&self, query_ids: &[String]) -> Vec<Fold> {
        let mut sorted_query_ids = query_ids.to_vec();
        sorted_query_ids.sort();

        let mut folds = Vec::with_capacity(self.k);
        for fold_idx in 0..self.k {
            folds.push(Fold {
                fold_idx,
                train_queries: Vec::new(),
                val_queries: Vec::new(),
            });
        }

        for (idx, qid) in sorted_query_ids.iter().enumerate() {
            let val_fold_idx = idx % self.k;
            for (f_idx, fold) in folds.iter_mut().enumerate() {
                if f_idx == val_fold_idx {
                    fold.val_queries.push(qid.clone());
                } else {
                    fold.train_queries.push(qid.clone());
                }
            }
        }

        for fold in &mut folds {
            fold.train_queries.sort();
            fold.val_queries.sort();
        }

        folds
    }
}

/// Evaluation result metrics for a single fold evaluation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoldEvaluationResult {
    /// Index of the fold.
    pub fold_idx: usize,
    /// Composite score.
    pub composite: f64,
    /// nDCG@5 score.
    pub ndcg_at_5: f64,
    /// MRR score.
    pub mrr: f64,
    /// Recall@5 score.
    pub recall_at_5: f64,
}

/// The metric variants supported by cross-validation summaries.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum EvaluationMetric {
    /// The Composite objective score.
    Composite,
    /// The nDCG@5 ranking score.
    NdcgAt5,
    /// Mean Reciprocal Rank.
    Mrr,
    /// Recall at cutoff 5.
    RecallAt5,
}

/// Metric statistics distribution across all folds.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricDistribution {
    /// Arithmetic mean.
    pub mean: f64,
    /// Sample standard deviation.
    pub std_dev: f64,
    /// Minimum value seen.
    pub min: f64,
    /// Maximum value seen.
    pub max: f64,
}

/// Summary metrics maps for cross-validation runs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrossValidationSummary {
    /// Distributions map keyed by metric.
    pub distributions: BTreeMap<EvaluationMetric, MetricDistribution>,
}

/// Complete cross-validation result report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrossValidationResult {
    /// Fold count K.
    pub k: usize,
    /// Results for each of the K folds.
    pub folds: Vec<FoldEvaluationResult>,
    /// Summary distributions aggregate statistics.
    pub summary: CrossValidationSummary,
}

/// Helper to compute summary statistics for a vector of values.
pub fn compute_distribution(values: &[f64]) -> MetricDistribution {
    let n = values.len();
    if n == 0 {
        return MetricDistribution {
            mean: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 0.0,
        };
    }
    let mean = values.iter().sum::<f64>() / (n as f64);
    let var = if n > 1 {
        values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / ((n - 1) as f64)
    } else {
        0.0
    };
    MetricDistribution {
        mean,
        std_dev: var.sqrt(),
        min: values.iter().copied().fold(f64::INFINITY, f64::min),
        max: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    }
}

/// Runner executing cross-validation.
pub struct CrossValidationRunner;

impl CrossValidationRunner {
    /// Runs K-Fold cross-validation on LambdaMART.
    pub fn run(
        session: &EvaluationSession,
        dataset: &TrainingDataset,
        config: &LambdaMartTrainingConfig,
        assigner: &FoldAssigner,
    ) -> Result<CrossValidationResult, BrainError> {
        let query_ids: Vec<String> = session.cache.iter().map(|c| c.query_id.clone()).collect();
        let folds = assigner.assign(&query_ids);

        let mut fold_results = Vec::with_capacity(folds.len());

        for fold in &folds {
            let train_set: HashSet<String> = fold.train_queries.iter().cloned().collect();
            let val_set: HashSet<String> = fold.val_queries.iter().cloned().collect();

            // Filter training dataset for the train queries of this fold
            let fold_train_examples: Vec<_> = dataset
                .examples
                .iter()
                .filter(|ex| train_set.contains(&ex.query_id))
                .cloned()
                .collect();
            let fold_train_dataset = TrainingDataset {
                examples: fold_train_examples,
            };

            // Internal train/validation split within train_queries (80/20 split) to avoid leaking the held-out validation queries
            let mut sorted_train_queries = fold.train_queries.clone();
            sorted_train_queries.sort();
            let sub_val_count = ((sorted_train_queries.len() as f64) * 0.20).round() as usize;
            let sub_train_count = sorted_train_queries.len() - sub_val_count;

            let sub_train_queries: HashSet<String> = sorted_train_queries[0..sub_train_count]
                .iter()
                .cloned()
                .collect();
            let sub_val_queries: HashSet<String> = sorted_train_queries[sub_train_count..]
                .iter()
                .cloned()
                .collect();

            // Build temporary dataset for internal GBDT selection
            let sub_train_examples: Vec<_> = fold_train_dataset
                .examples
                .iter()
                .filter(|ex| {
                    sub_train_queries.contains(&ex.query_id)
                        || sub_val_queries.contains(&ex.query_id)
                })
                .cloned()
                .collect();
            let sub_dataset = TrainingDataset {
                examples: sub_train_examples,
            };

            // Train model with sub_val split
            let sub_config = LambdaMartTrainingConfig {
                num_trees: config.num_trees,
                max_depth: config.max_depth,
                learning_rate: config.learning_rate,
                min_samples_split: config.min_samples_split,
                validation_fraction: 0.20,
            };

            let history = LambdaMartTrainer::train(&sub_dataset, &sub_config)?;
            let selection = ModelSelector::select_best(&history);
            let lambdamart_model = LambdaMartModel::from_history(&history, &selection);

            // Evaluate on the held-out validation fold
            let (mean_ndcg, mean_mrr, composite) =
                evaluate_fold_metrics(session, &lambdamart_model, &val_set);

            // Compute Recall@5
            let mut sum_recall = 0.0;
            let mut count = 0;
            let extractor = FeatureExtractor::new(session.reference_time, session.decay);
            for query_cache in &session.cache {
                if !val_set.contains(&query_cache.query_id) {
                    continue;
                }
                count += 1;
                let mut scored_results = Vec::new();
                for (res, ctx) in &query_cache.candidates {
                    let features = extractor.extract(res, ctx);
                    let score = lambdamart_model.score(&features);
                    let mut cloned_res = res.clone();
                    cloned_res.ranking_score = Some(score);
                    scored_results.push(cloned_res);
                }
                crate::retrieval::eval_harness::sort_results_deterministically(&mut scored_results);
                let retrieved_ids: Vec<NodeId> = scored_results.iter().map(|r| r.node_id).collect();
                sum_recall += crate::retrieval::eval_harness::metrics::compute_recall_at_k(
                    &retrieved_ids,
                    &query_cache.expected_node_ids,
                    5,
                );
            }
            let mean_recall = if count == 0 {
                0.0
            } else {
                sum_recall / (count as f64)
            };

            fold_results.push(FoldEvaluationResult {
                fold_idx: fold.fold_idx,
                composite,
                ndcg_at_5: mean_ndcg,
                mrr: mean_mrr,
                recall_at_5: mean_recall,
            });
        }

        // 3. Compute metric distributions
        let composites: Vec<f64> = fold_results.iter().map(|f| f.composite).collect();
        let ndcgs: Vec<f64> = fold_results.iter().map(|f| f.ndcg_at_5).collect();
        let mrrs: Vec<f64> = fold_results.iter().map(|f| f.mrr).collect();
        let recalls: Vec<f64> = fold_results.iter().map(|f| f.recall_at_5).collect();

        let mut distributions = BTreeMap::new();
        distributions.insert(
            EvaluationMetric::Composite,
            compute_distribution(&composites),
        );
        distributions.insert(EvaluationMetric::NdcgAt5, compute_distribution(&ndcgs));
        distributions.insert(EvaluationMetric::Mrr, compute_distribution(&mrrs));
        distributions.insert(EvaluationMetric::RecallAt5, compute_distribution(&recalls));

        let summary = CrossValidationSummary { distributions };

        Ok(CrossValidationResult {
            k: assigner.k,
            folds: fold_results,
            summary,
        })
    }
}

fn evaluate_fold_metrics<M: ScoreRanker>(
    session: &EvaluationSession,
    model: &M,
    query_ids: &HashSet<String>,
) -> (f64, f64, f64) {
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
        crate::retrieval::eval_harness::sort_results_deterministically(&mut scored_results);
        let retrieved_ids: Vec<NodeId> = scored_results.iter().map(|r| r.node_id).collect();

        sum_recall += crate::retrieval::eval_harness::metrics::compute_recall_at_k(
            &retrieved_ids,
            &query_cache.expected_node_ids,
            5,
        );
        sum_mrr += crate::retrieval::eval_harness::metrics::compute_mrr(
            &retrieved_ids,
            &query_cache.expected_node_ids,
            &query_cache.acceptable_alternatives,
        );
        sum_ndcg += crate::retrieval::eval_harness::metrics::compute_ndcg_at_k(
            &retrieved_ids,
            &query_cache.expected_node_ids,
            &query_cache.acceptable_alternatives,
            5,
        );
    }

    if count == 0 {
        return (0.0, 0.0, 0.0);
    }

    let mean_ndcg = sum_ndcg / (count as f64);
    let mean_mrr = sum_mrr / (count as f64);
    let mean_recall = sum_recall / (count as f64);
    let composite = 0.60 * mean_ndcg + 0.20 * mean_mrr + 0.20 * mean_recall;

    (mean_ndcg, mean_mrr, composite)
}
