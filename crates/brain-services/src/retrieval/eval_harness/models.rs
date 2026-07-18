use crate::retrieval::eval_harness::{
    EvaluationSession, Feature, FeatureExtractor, RankingWeights,
};
use crate::retrieval::ranking::feature_provider::FeatureVector;
pub use crate::retrieval::ranking::score_ranker::ScoreRanker;
use brain_core::errors::BrainError;
use std::collections::{HashMap, HashSet};

/// A single training example for supervised rankers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingExample {
    /// Unique query identifier grouping candidate examples.
    pub query_id: String,
    /// Feature vector extracted for the candidate node.
    pub features: FeatureVector,
    /// Graded relevance score (1.0 = primary expected, 0.5 = acceptable alternative, 0.0 = irrelevant).
    pub relevance: f32,
}

impl TrainingExample {
    /// Helper to identify if the node has any positive relevance to the query.
    pub fn is_relevant(&self) -> bool {
        self.relevance > 0.0
    }
}

/// Supervised dataset containing multiple training examples.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingDataset {
    /// Collection of examples.
    pub examples: Vec<TrainingExample>,
}

impl TrainingDataset {
    /// Generates a training dataset from an EvaluationSession.
    pub fn from_session(session: &EvaluationSession) -> Self {
        let extractor = FeatureExtractor::new(session.reference_time, session.decay);
        let mut examples = Vec::new();

        for query_cache in &session.cache {
            let expected_set: HashSet<brain_domain::NodeId> =
                query_cache.expected_node_ids.iter().cloned().collect();
            let acceptable_set: HashSet<brain_domain::NodeId> = query_cache
                .acceptable_alternatives
                .iter()
                .cloned()
                .collect();

            for (res, ctx) in &query_cache.candidates {
                let features = extractor.extract(res, ctx);
                let relevance = if expected_set.contains(&res.node_id) {
                    1.0
                } else if acceptable_set.contains(&res.node_id) {
                    0.5
                } else {
                    0.0
                };
                examples.push(TrainingExample {
                    query_id: query_cache.query_id.clone(),
                    features,
                    relevance,
                });
            }
        }

        Self { examples }
    }
}

/// An immutable trained Logistic Regression model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogisticRegressionModel {
    /// Trained weights configuration.
    pub weights: RankingWeights,
    /// Model bias / intercept parameter.
    pub intercept: f64,
}

impl ScoreRanker for LogisticRegressionModel {
    fn name(&self) -> &'static str {
        "logistic-regression-model"
    }

    fn score(&self, features: &FeatureVector) -> f64 {
        let mut z = self.intercept;
        if let Some(lex) = features.lexical_similarity {
            z += lex * self.weights.lexical;
        }
        if let Some(sem) = features.semantic_similarity {
            z += sem * self.weights.semantic;
        }
        if let Some(rec) = features.recency {
            z += rec * self.weights.recency;
        }
        if let Some(imp) = features.importance {
            z += imp * self.weights.importance;
        }
        if let Some(prov) = features.provenance_confidence {
            z += prov * self.weights.provenance_confidence;
        }
        if let Some(graph) = features.graph_degree {
            z += graph * self.weights.graph_degree;
        }
        if let Some(acc) = features.access_frequency {
            z += acc * self.weights.access_frequency;
        }
        if let Some(fresh) = features.freshness_decay {
            z += fresh * self.weights.freshness_decay;
        }

        // Sigmoid probability mapping
        1.0 / (1.0 + (-z).exp())
    }
}

/// Hyperparameters for logistic regression training.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogisticTrainingConfig {
    /// Gradient descent learning rate (step size).
    pub learning_rate: f64,
    /// Number of complete iterations over the dataset.
    pub epochs: usize,
    /// L2 regularization multiplier.
    pub l2_regularization: f64,
    /// Optional difference threshold to stop early if BCE loss change is negligible.
    pub convergence_tolerance: Option<f64>,
}

/// Optimization run metadata diagnostics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingSummary {
    /// Initial Binary Cross-Entropy loss before training.
    pub initial_loss: f64,
    /// Final Binary Cross-Entropy loss after training.
    pub final_loss: f64,
    /// Total number of epochs executed.
    pub epochs_run: usize,
    /// Whether the training met convergence criteria.
    pub converged: bool,
}

/// Trainer utilizing batch gradient descent to fit a Logistic Regression model.
pub struct LogisticTrainer;

impl LogisticTrainer {
    /// Deterministically trains a LogisticRegressionModel using batch gradient descent.
    pub fn train(
        dataset: &TrainingDataset,
        config: &LogisticTrainingConfig,
    ) -> Result<(LogisticRegressionModel, TrainingSummary), BrainError> {
        let n = dataset.examples.len();
        if n == 0 {
            return Err(BrainError::Validation {
                message: "Cannot train logistic regression on empty dataset.".to_string(),
            });
        }

        // Initialize parameters to 0.0
        let mut weights = [0.0; 8];
        let mut intercept = 0.0;

        let get_x = |ex: &TrainingExample| -> [f64; 8] {
            [
                ex.features.access_frequency.unwrap_or(0.0),
                ex.features.freshness_decay.unwrap_or(0.0),
                ex.features.graph_degree.unwrap_or(0.0),
                ex.features.importance.unwrap_or(0.0),
                ex.features.lexical_similarity.unwrap_or(0.0),
                ex.features.provenance_confidence.unwrap_or(0.0),
                ex.features.recency.unwrap_or(0.0),
                ex.features.semantic_similarity.unwrap_or(0.0),
            ]
        };

        let compute_p = |x: &[f64; 8], w: &[f64; 8], b: f64| -> f64 {
            let mut z = b;
            for i in 0..8 {
                z += x[i] * w[i];
            }
            1.0 / (1.0 + (-z).exp())
        };

        let compute_loss = |w: &[f64; 8], b: f64| -> f64 {
            let mut sum = 0.0;
            for ex in &dataset.examples {
                let x = get_x(ex);
                let p = compute_p(&x, w, b);
                let y = if ex.is_relevant() { 1.0 } else { 0.0 };
                let p_clipped = p.clamp(1e-15, 1.0 - 1e-15);
                sum += y * p_clipped.ln() + (1.0 - y) * (1.0 - p_clipped).ln();
            }

            let mut reg = 0.0;
            for &weight in w {
                reg += weight * weight;
            }

            -(sum / (n as f64)) + 0.5 * config.l2_regularization * reg
        };

        let initial_loss = compute_loss(&weights, intercept);
        let mut prev_loss = initial_loss;
        let mut epochs_run = 0;
        let mut converged = false;

        for _epoch in 0..config.epochs {
            let mut grad_w = [0.0; 8];
            let mut grad_b = 0.0;

            for ex in &dataset.examples {
                let x = get_x(ex);
                let p = compute_p(&x, &weights, intercept);
                let y = if ex.is_relevant() { 1.0 } else { 0.0 };
                let diff = p - y;

                for i in 0..8 {
                    grad_w[i] += diff * x[i];
                }
                grad_b += diff;
            }

            for i in 0..8 {
                grad_w[i] = (grad_w[i] / (n as f64)) + config.l2_regularization * weights[i];
            }
            grad_b /= n as f64;

            for i in 0..8 {
                weights[i] -= config.learning_rate * grad_w[i];
            }
            intercept -= config.learning_rate * grad_b;

            epochs_run += 1;

            let current_loss = compute_loss(&weights, intercept);

            if let Some(tolerance) = config.convergence_tolerance {
                if (prev_loss - current_loss).abs() < tolerance {
                    converged = true;
                    break;
                }
            }
            prev_loss = current_loss;
        }

        let final_loss = prev_loss;

        let ranking_weights = RankingWeights {
            access_frequency: weights[0],
            freshness_decay: weights[1],
            graph_degree: weights[2],
            importance: weights[3],
            lexical: weights[4],
            provenance_confidence: weights[5],
            recency: weights[6],
            semantic: weights[7],
        };

        let model = LogisticRegressionModel {
            weights: ranking_weights,
            intercept,
        };

        let summary = TrainingSummary {
            initial_loss,
            final_loss,
            epochs_run,
            converged,
        };

        Ok((model, summary))
    }
}

/// A generic regression dataset holding features and targets, fully decoupled from retrieval concepts.
#[derive(Debug, Clone)]
pub struct RegressionDataset {
    /// Feature vectors matrix (N x M).
    pub features: Vec<Vec<f64>>,
    /// Target regression values (N).
    pub targets: Vec<f64>,
}

/// Explicit adapter transforming domain TrainingExamples to numeric RegressionDatasets.
pub struct RegressionDatasetBuilder;

impl RegressionDatasetBuilder {
    /// Builds a RegressionDataset from a subset of examples and targets defined by indices.
    pub fn from_examples(
        examples: &[TrainingExample],
        targets: &[f64],
        include_indices: &[usize],
    ) -> RegressionDataset {
        let mut features = Vec::with_capacity(include_indices.len());
        let mut subset_targets = Vec::with_capacity(include_indices.len());
        for &idx in include_indices {
            let ex = &examples[idx];
            features.push(vec![
                ex.features.access_frequency.unwrap_or(0.0),
                ex.features.freshness_decay.unwrap_or(0.0),
                ex.features.graph_degree.unwrap_or(0.0),
                ex.features.importance.unwrap_or(0.0),
                ex.features.lexical_similarity.unwrap_or(0.0),
                ex.features.provenance_confidence.unwrap_or(0.0),
                ex.features.recency.unwrap_or(0.0),
                ex.features.semantic_similarity.unwrap_or(0.0),
            ]);
            subset_targets.push(targets[idx]);
        }
        RegressionDataset {
            features,
            targets: subset_targets,
        }
    }
}

/// A node representation in a generic Regression Tree.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum TreeNode {
    /// Leaf node predicting a constant value.
    Leaf {
        /// The predicted output.
        value: f64,
    },
    /// Split node branching on a feature threshold.
    Split {
        /// Index of the feature to split on.
        feature_idx: usize,
        /// Feature threshold split value.
        split_value: f64,
        /// SSE error reduction score.
        split_gain: f64,
        /// Left child branch (<= split_value).
        left: Box<TreeNode>,
        /// Right child branch (> split_value).
        right: Box<TreeNode>,
    },
}

/// Reusable regression CART tree ensemble component.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RegressionTree {
    /// The root node of the CART tree.
    pub root: TreeNode,
}

impl RegressionTree {
    /// Evaluates the features vector to predict the continuous target value.
    pub fn predict(&self, features: &[f64]) -> f64 {
        Self::predict_node(&self.root, features)
    }

    fn predict_node(node: &TreeNode, features: &[f64]) -> f64 {
        match node {
            TreeNode::Leaf { value } => *value,
            TreeNode::Split {
                feature_idx,
                split_value,
                left,
                right,
                ..
            } => {
                let val = features[*feature_idx];
                if val <= *split_value {
                    Self::predict_node(left, features)
                } else {
                    Self::predict_node(right, features)
                }
            }
        }
    }
}

/// Reusable Regression Tree Trainer utilizing Mean Squared Error minimization.
pub struct RegressionTreeTrainer {
    /// Maximum tree depth constraint.
    pub max_depth: usize,
    /// Minimum samples required to split a node.
    pub min_samples_split: usize,
}

fn compute_sse_for_idxs(dataset: &RegressionDataset, idxs: &[usize]) -> f64 {
    let n = idxs.len();
    if n == 0 {
        return 0.0;
    }
    let sum: f64 = idxs.iter().map(|&i| dataset.targets[i]).sum();
    let mean = sum / (n as f64);
    idxs.iter()
        .map(|&i| (dataset.targets[i] - mean).powi(2))
        .sum()
}

impl RegressionTreeTrainer {
    /// Builds a RegressionTree by greedily splitting on the optimal MSE threshold.
    pub fn fit(&self, dataset: &RegressionDataset) -> RegressionTree {
        let idxs: Vec<usize> = (0..dataset.features.len()).collect();
        let root = self.build_node(dataset, idxs, 0);
        RegressionTree { root }
    }

    fn build_node(&self, dataset: &RegressionDataset, idxs: Vec<usize>, depth: usize) -> TreeNode {
        let n = idxs.len();
        if n == 0 {
            return TreeNode::Leaf { value: 0.0 };
        }

        let sum_y: f64 = idxs.iter().map(|&i| dataset.targets[i]).sum();
        let mean_y = sum_y / (n as f64);

        if depth >= self.max_depth || n < self.min_samples_split {
            return TreeNode::Leaf { value: mean_y };
        }

        let mut best_feature = 0;
        let mut best_split_value = 0.0;
        let mut best_mse = f64::INFINITY;
        let mut split_found = false;

        for feature_idx in 0..8 {
            let mut values: Vec<(f64, f64)> = idxs
                .iter()
                .map(|&i| (dataset.features[i][feature_idx], dataset.targets[i]))
                .collect();
            values.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            let mut sum_total = 0.0;
            let mut sum_sq_total = 0.0;
            for &(_, target) in &values {
                sum_total += target;
                sum_sq_total += target * target;
            }

            let mut sum_left = 0.0;
            let mut sum_sq_left = 0.0;

            for j in 0..(n - 1) {
                let val_j = values[j].0;
                let val_j1 = values[j + 1].0;

                sum_left += values[j].1;
                sum_sq_left += values[j].1 * values[j].1;

                if val_j == val_j1 {
                    continue;
                }

                let split_val = 0.5 * (val_j + val_j1);
                let n_left = (j + 1) as f64;
                let n_right = (n - j - 1) as f64;

                let sum_right = sum_total - sum_left;
                let sum_sq_right = sum_sq_total - sum_sq_left;

                let sse_left = (sum_sq_left - (sum_left * sum_left) / n_left).max(0.0);
                let sse_right = (sum_sq_right - (sum_right * sum_right) / n_right).max(0.0);
                let mse = sse_left + sse_right;

                if mse < best_mse {
                    best_mse = mse;
                    best_feature = feature_idx;
                    best_split_value = split_val;
                    split_found = true;
                }
            }
        }

        if !split_found {
            return TreeNode::Leaf { value: mean_y };
        }

        let mut best_left = Vec::new();
        let mut best_right = Vec::new();
        for &idx in &idxs {
            let val = dataset.features[idx][best_feature];
            if val <= best_split_value {
                best_left.push(idx);
            } else {
                best_right.push(idx);
            }
        }

        let sse_parent = compute_sse_for_idxs(dataset, &idxs);
        let sse_left = compute_sse_for_idxs(dataset, &best_left);
        let sse_right = compute_sse_for_idxs(dataset, &best_right);
        let split_gain = sse_parent - (sse_left + sse_right);

        let left_child = self.build_node(dataset, best_left, depth + 1);
        let right_child = self.build_node(dataset, best_right, depth + 1);

        TreeNode::Split {
            feature_idx: best_feature,
            split_value: best_split_value,
            split_gain,
            left: Box::new(left_child),
            right: Box::new(right_child),
        }
    }
}

/// Isolated calculator computing listwise LambdaRank gradients.
pub struct LambdaGradientComputer {
    /// Gradient scaling hyperparameter.
    pub sigma: f64,
}

impl LambdaGradientComputer {
    /// Computes lambda gradients for candidates of a single query group.
    pub fn compute(&self, relevance_labels: &[f32], scores: &[f64]) -> Vec<f64> {
        let n = relevance_labels.len();
        let mut lambdas = vec![0.0; n];
        if n <= 1 {
            return lambdas;
        }

        // 1. Sort indices by score descending, breaking ties stably
        let mut idxs: Vec<usize> = (0..n).collect();
        idxs.sort_by(|&a, &b| {
            scores[b]
                .partial_cmp(&scores[a])
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(&b))
        });

        // 2. Compute Ideal DCG (IDCG) at all positions
        let mut sorted_relevances = relevance_labels.to_vec();
        sorted_relevances.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let mut idcg = 0.0;
        for (r, &rel) in sorted_relevances.iter().enumerate() {
            let gain = (2.0f64.powf(rel as f64)) - 1.0;
            let disc = ((r + 2) as f64).log2();
            idcg += gain / disc;
        }

        if idcg <= 1e-9 {
            return lambdas;
        }

        let mut ranks = vec![0; n];
        for (r, &idx) in idxs.iter().enumerate() {
            ranks[idx] = r + 1;
        }

        // 3. Compute pairwise lambdas
        for i in 0..n {
            for j in 0..n {
                if relevance_labels[i] > relevance_labels[j] {
                    let r_i = ranks[i];
                    let r_j = ranks[j];

                    let gain_i = (2.0f64.powf(relevance_labels[i] as f64)) - 1.0;
                    let gain_j = (2.0f64.powf(relevance_labels[j] as f64)) - 1.0;

                    let disc_i = (r_i + 1) as f64;
                    let disc_j = (r_j + 1) as f64;

                    let swap_dcg = (gain_i / disc_j.log2()) + (gain_j / disc_i.log2());
                    let orig_dcg = (gain_i / disc_i.log2()) + (gain_j / disc_j.log2());
                    let delta_ndcg = (swap_dcg - orig_dcg).abs() / idcg;

                    let score_diff = scores[i] - scores[j];
                    let lambda_ij = (1.0 / (1.0 + (self.sigma * score_diff).exp())) * delta_ndcg;

                    lambdas[i] += lambda_ij;
                    lambdas[j] -= lambda_ij;
                }
            }
        }

        lambdas
    }
}

/// Configuration parameters for LambdaMART training.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LambdaMartTrainingConfig {
    /// Number of boosted regression trees.
    pub num_trees: usize,
    /// Maximum depth of each regression tree.
    pub max_depth: usize,
    /// Shrinkage/learning rate multiplier.
    pub learning_rate: f64,
    /// Minimum samples required to split.
    pub min_samples_split: usize,
    /// Fraction of training query groups to split into validation set (0.0 means no validation).
    pub validation_fraction: f64,
}

/// Metadata diagnostics of the trained LambdaMART ensemble.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct LambdaMartMetadata {
    /// Total trees trained.
    pub num_trees: usize,
    /// Max depth limit.
    pub max_depth: usize,
    /// Learning rate scaling.
    pub learning_rate: f64,
    /// Number of training queries.
    pub training_queries: usize,
}

/// Diagnostic metrics calculated during GBDT epochs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EpochMetrics {
    /// Boosting round epoch.
    pub epoch: usize,
    /// Average nDCG on training queries.
    pub train_ndcg: f64,
    /// Average nDCG on validation queries.
    pub validation_ndcg: f64,
    /// Mean lambda gradient magnitude.
    pub mean_lambda_magnitude: f64,
}

/// Reason indicating how the model selection concluded.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum SelectionReason {
    /// Epoch exhibiting highest validation nDCG.
    PeakValidationNdcg,
    /// Limit reached because validation fraction was 0.0 or validation nDCG never improved.
    MaxEpochLimitReached,
}

/// Selection result detailing the selected best epoch.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelSelectionResult {
    /// 0-based index of the best epoch.
    pub best_epoch: usize,
    /// The selection decision path rationale.
    pub reason: SelectionReason,
}

/// Immutable record of the complete boosting run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingHistory {
    epochs: Vec<EpochMetrics>,
    trees: Vec<RegressionTree>,
    /// Score intercept baseline.
    pub initial_score: f64,
    /// Calibration metadata context.
    pub metadata: LambdaMartMetadata,
}

impl TrainingHistory {
    /// Accessor for read-only epochs.
    pub fn epochs(&self) -> &[EpochMetrics] {
        &self.epochs
    }

    /// Accessor for read-only trees.
    pub fn trees(&self) -> &[RegressionTree] {
        &self.trees
    }
}

/// Selector structure evaluating boosting metrics to pick the best epoch.
pub struct ModelSelector;

impl ModelSelector {
    /// Selects the best epoch from training history using peak validation nDCG.
    pub fn select_best(history: &TrainingHistory) -> ModelSelectionResult {
        if history.epochs.is_empty() {
            return ModelSelectionResult {
                best_epoch: 0,
                reason: SelectionReason::MaxEpochLimitReached,
            };
        }

        let mut best_epoch = 0;
        let mut best_val = -1.0;
        for (idx, epoch) in history.epochs.iter().enumerate() {
            if epoch.validation_ndcg > best_val + 1e-9 {
                best_val = epoch.validation_ndcg;
                best_epoch = idx;
            }
        }

        let reason = if best_epoch == history.epochs.len() - 1 {
            SelectionReason::MaxEpochLimitReached
        } else {
            SelectionReason::PeakValidationNdcg
        };

        ModelSelectionResult { best_epoch, reason }
    }
}

/// Reusable LambdaMART model ensemble implementing ScoreRanker.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct LambdaMartModel {
    /// Boosted decision trees.
    pub trees: Vec<RegressionTree>,
    /// Shrinkage factor.
    pub learning_rate: f64,
    /// Initial baseline prediction score.
    pub initial_score: f64,
    /// Ensemble metadata.
    pub metadata: LambdaMartMetadata,
}

impl LambdaMartModel {
    /// Slices and returns a LambdaMartModel pruned to the specified best selection result.
    pub fn from_history(history: &TrainingHistory, selection: &ModelSelectionResult) -> Self {
        let count = selection.best_epoch + 1;
        let mut trees_to_keep = Vec::new();
        if count <= history.trees.len() {
            trees_to_keep = history.trees[0..count].to_vec();
        }
        let mut metadata = history.metadata.clone();
        metadata.num_trees = trees_to_keep.len();

        Self {
            trees: trees_to_keep,
            learning_rate: history.metadata.learning_rate,
            initial_score: history.initial_score,
            metadata,
        }
    }
}

impl ScoreRanker for LambdaMartModel {
    fn name(&self) -> &'static str {
        "lambdamart-model"
    }

    fn score(&self, features: &FeatureVector) -> f64 {
        let mut score = self.initial_score;
        let x = [
            features.access_frequency.unwrap_or(0.0),
            features.freshness_decay.unwrap_or(0.0),
            features.graph_degree.unwrap_or(0.0),
            features.importance.unwrap_or(0.0),
            features.lexical_similarity.unwrap_or(0.0),
            features.provenance_confidence.unwrap_or(0.0),
            features.recency.unwrap_or(0.0),
            features.semantic_similarity.unwrap_or(0.0),
        ];
        for tree in &self.trees {
            score += self.learning_rate * tree.predict(&x);
        }
        score
    }
}

/// Trainer implementing listwise gradient boosting (LambdaRank + MART) with separate model selection hooks.
pub struct LambdaMartTrainer;

impl LambdaMartTrainer {
    /// Trains a LambdaMartModel ensemble, returning the full immutable TrainingHistory.
    pub fn train(
        dataset: &TrainingDataset,
        config: &LambdaMartTrainingConfig,
    ) -> Result<TrainingHistory, BrainError> {
        let n = dataset.examples.len();
        if n == 0 {
            return Err(BrainError::Validation {
                message: "Cannot train LambdaMART on an empty dataset.".to_string(),
            });
        }

        // Group training example indices by query_id
        let mut query_groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, ex) in dataset.examples.iter().enumerate() {
            query_groups
                .entry(ex.query_id.clone())
                .or_default()
                .push(idx);
        }

        let num_queries = query_groups.len();
        if num_queries == 0 {
            return Err(BrainError::Validation {
                message: "Cannot train LambdaMART with 0 queries.".to_string(),
            });
        }

        // Deterministic split: sort queries alphabetically
        let mut sorted_queries: Vec<String> = query_groups.keys().cloned().collect();
        sorted_queries.sort();

        let val_queries_count =
            ((sorted_queries.len() as f64) * config.validation_fraction).round() as usize;
        let train_queries_count = sorted_queries.len() - val_queries_count;

        let train_queries: HashSet<String> = sorted_queries[0..train_queries_count]
            .iter()
            .cloned()
            .collect();
        let val_queries: HashSet<String> = sorted_queries[train_queries_count..]
            .iter()
            .cloned()
            .collect();

        // Build index list of training examples
        let mut train_example_indices = Vec::new();
        for (idx, ex) in dataset.examples.iter().enumerate() {
            if train_queries.contains(&ex.query_id) {
                train_example_indices.push(idx);
            }
        }

        let initial_score = 0.0;
        let mut current_scores = vec![initial_score; n];

        let mut trees = Vec::with_capacity(config.num_trees);
        let mut epochs = Vec::with_capacity(config.num_trees);
        let computer = LambdaGradientComputer { sigma: 1.0 };
        let tree_trainer = RegressionTreeTrainer {
            max_depth: config.max_depth,
            min_samples_split: config.min_samples_split,
        };

        // Helper to compute mean nDCG across query subsets
        let compute_subset_ndcg = |scores: &[f64], query_set: &HashSet<String>| -> f64 {
            let mut sum_ndcg = 0.0;
            let mut subset_count = 0;

            for (qid, indices) in &query_groups {
                if !query_set.contains(qid) {
                    continue;
                }
                subset_count += 1;

                let mut sorted_indices = indices.clone();
                sorted_indices.sort_by(|&a, &b| {
                    scores[b]
                        .partial_cmp(&scores[a])
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.cmp(&b))
                });

                let mut sorted_relevances: Vec<f64> = indices
                    .iter()
                    .map(|&idx| dataset.examples[idx].relevance as f64)
                    .collect();
                sorted_relevances
                    .sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

                let mut idcg = 0.0;
                for (r, &rel) in sorted_relevances.iter().enumerate() {
                    idcg += ((2.0f64.powf(rel)) - 1.0) / ((r + 2) as f64).log2();
                }

                if idcg > 0.0 {
                    let mut dcg = 0.0;
                    for (r, &idx) in sorted_indices.iter().enumerate() {
                        let rel = dataset.examples[idx].relevance as f64;
                        dcg += ((2.0f64.powf(rel)) - 1.0) / ((r + 2) as f64).log2();
                    }
                    sum_ndcg += dcg / idcg;
                }
            }

            if subset_count == 0 {
                0.0
            } else {
                sum_ndcg / (subset_count as f64)
            }
        };

        for epoch in 0..config.num_trees {
            let mut targets = vec![0.0; n];
            let mut sum_lambda_magnitude = 0.0;
            let mut train_lambda_count = 0;

            // 1. Compute lambda gradients ONLY for training queries
            for (qid, indices) in &query_groups {
                if !train_queries.contains(qid) {
                    continue;
                }

                let relevances: Vec<f32> = indices
                    .iter()
                    .map(|&idx| dataset.examples[idx].relevance)
                    .collect();
                let q_scores: Vec<f64> = indices.iter().map(|&idx| current_scores[idx]).collect();

                let lambdas = computer.compute(&relevances, &q_scores);
                for (local_idx, &lambda) in lambdas.iter().enumerate() {
                    let global_idx = indices[local_idx];
                    targets[global_idx] = lambda;
                    sum_lambda_magnitude += lambda.abs();
                    train_lambda_count += 1;
                }
            }

            // 2. Build regression dataset from training partition only and train tree
            let reg_dataset = RegressionDatasetBuilder::from_examples(
                &dataset.examples,
                &targets,
                &train_example_indices,
            );
            let tree = tree_trainer.fit(&reg_dataset);

            // 3. Update current predictions for ALL examples (train + validation)
            for i in 0..n {
                let x = [
                    dataset.examples[i].features.access_frequency.unwrap_or(0.0),
                    dataset.examples[i].features.freshness_decay.unwrap_or(0.0),
                    dataset.examples[i].features.graph_degree.unwrap_or(0.0),
                    dataset.examples[i].features.importance.unwrap_or(0.0),
                    dataset.examples[i]
                        .features
                        .lexical_similarity
                        .unwrap_or(0.0),
                    dataset.examples[i]
                        .features
                        .provenance_confidence
                        .unwrap_or(0.0),
                    dataset.examples[i].features.recency.unwrap_or(0.0),
                    dataset.examples[i]
                        .features
                        .semantic_similarity
                        .unwrap_or(0.0),
                ];
                current_scores[i] += config.learning_rate * tree.predict(&x);
            }

            // 4. Compute metrics at the end of epoch
            let train_ndcg = compute_subset_ndcg(&current_scores, &train_queries);
            let validation_ndcg = if val_queries.is_empty() {
                train_ndcg // fallback to train if validation ratio is 0.0
            } else {
                compute_subset_ndcg(&current_scores, &val_queries)
            };

            let mean_lambda = if train_lambda_count == 0 {
                0.0
            } else {
                sum_lambda_magnitude / (train_lambda_count as f64)
            };

            epochs.push(EpochMetrics {
                epoch,
                train_ndcg,
                validation_ndcg,
                mean_lambda_magnitude: mean_lambda,
            });

            trees.push(tree);
        }

        let metadata = LambdaMartMetadata {
            num_trees: config.num_trees,
            max_depth: config.max_depth,
            learning_rate: config.learning_rate,
            training_queries: train_queries_count,
        };

        Ok(TrainingHistory {
            epochs,
            trees,
            initial_score,
            metadata,
        })
    }
}

/// A strongly typed entry representing normalized gain score for a specific Feature.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FeatureImportance {
    /// The specific feature enum variant.
    pub feature: Feature,
    /// Calculated continuous importance/gain score.
    pub gain: f64,
}

/// Explicit analysis report containing entries for feature importances.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FeatureImportanceReport {
    /// Ordered importance report entries.
    pub entries: Vec<FeatureImportance>,
}

/// Dedicated analyzer computing gain importance metrics from decision tree splits.
pub struct FeatureImportanceAnalyzer;

impl FeatureImportanceAnalyzer {
    /// Analyzes a LambdaMartModel and returns a FeatureImportanceReport.
    pub fn analyze(model: &LambdaMartModel) -> FeatureImportanceReport {
        let mut gains = [0.0f64; 8];

        fn traverse(node: &TreeNode, gains: &mut [f64; 8]) {
            match node {
                TreeNode::Leaf { .. } => {}
                TreeNode::Split {
                    feature_idx,
                    split_gain,
                    left,
                    right,
                    ..
                } => {
                    if *feature_idx < 8 {
                        gains[*feature_idx] += split_gain;
                    }
                    traverse(left, gains);
                    traverse(right, gains);
                }
            }
        }

        for tree in &model.trees {
            traverse(&tree.root, &mut gains);
        }

        let sum_gain: f64 = gains.iter().sum();
        let normalized_gains = if sum_gain > 1e-9 {
            let mut n_gains = [0.0; 8];
            for i in 0..8 {
                n_gains[i] = gains[i] / sum_gain;
            }
            n_gains
        } else {
            // Equal distribution if no splits were performed
            [0.125; 8]
        };

        let map_idx_to_feature = |idx: usize| -> Feature {
            match idx {
                0 => Feature::AccessFrequency,
                1 => Feature::FreshnessDecay,
                2 => Feature::GraphDegree,
                3 => Feature::Importance,
                4 => Feature::LexicalSimilarity,
                5 => Feature::ProvenanceConfidence,
                6 => Feature::Recency,
                7 => Feature::SemanticSimilarity,
                _ => Feature::LexicalSimilarity,
            }
        };

        let mut entries = Vec::with_capacity(8);
        for i in 0..8 {
            entries.push(FeatureImportance {
                feature: map_idx_to_feature(i),
                gain: normalized_gains[i],
            });
        }

        // Sort descending by gain to make reports readable
        entries.sort_by(|a, b| {
            b.gain
                .partial_cmp(&a.gain)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        FeatureImportanceReport { entries }
    }
}
