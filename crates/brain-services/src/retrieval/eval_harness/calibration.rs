use crate::retrieval::eval_harness::{
    metrics::{compute_mrr, compute_precision_at_k, compute_recall_at_k, compute_ndcg_at_k},
    FeatureContext, FeatureExtractor, FeatureVector, LinearRanker, RankingWeights, RetrievalResult,
    RetrievalChannel, Retriever, GroundTruthCorpus, QueryCorpus,
};
use brain_core::errors::BrainError;
use brain_domain::NodeId;
use std::collections::HashMap;

/// Calibration objective metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CalibrationObjective {
    /// Optimize for mean Normalized Discounted Cumulative Gain at K=5.
    NdcgAt5,
    /// Optimize for mean Normalized Discounted Cumulative Gain at K=10.
    NdcgAt10,
    /// Optimize for mean Mean Reciprocal Rank.
    Mrr,
    /// Optimize for mean Recall at K=5.
    RecallAt5,
    /// Optimize for mean Precision at K=5.
    PrecisionAt5,
    /// Optimize for versioned composite score: 0.60 * nDCG@5 + 0.20 * MRR + 0.20 * Recall@5.
    Composite,
}

impl CalibrationObjective {
    /// Computes target score based on objective function.
    pub fn score(&self, result: &CalibrationResult) -> f64 {
        match self {
            Self::NdcgAt5 => result.mean_ndcg_at_5,
            Self::NdcgAt10 => result.mean_ndcg_at_10,
            Self::Mrr => result.mean_mrr,
            Self::RecallAt5 => result.mean_recall_at_5,
            Self::PrecisionAt5 => result.mean_precision_at_5,
            Self::Composite => {
                0.60 * result.mean_ndcg_at_5
                    + 0.20 * result.mean_mrr
                    + 0.20 * result.mean_recall_at_5
            }
        }
    }
}

/// Search configurations parameters for optimization.
pub enum CalibrationOptions {
    /// Exhaustive grid search arrays.
    Grid {
        /// Coefficient array for lexical (FTS) similarity.
        lexical_weights: Vec<f64>,
        /// Coefficient array for vector semantic similarity.
        semantic_weights: Vec<f64>,
        /// Coefficient array for recency decay.
        recency_weights: Vec<f64>,
        /// Coefficient array for static importance.
        importance_weights: Vec<f64>,
        /// Coefficient array for provenance confidence.
        provenance_weights: Vec<f64>,
        /// Coefficient array for graph degree.
        graph_degree_weights: Vec<f64>,
        /// Coefficient array for access frequency.
        access_frequency_weights: Vec<f64>,
        /// Coefficient array for freshness decay.
        freshness_decay_weights: Vec<f64>,
    },
    /// Explicit subset of weight parameter candidates to evaluate.
    Candidates(Vec<RankingWeights>),
}

/// Aggregated metrics results for a specific calibration run.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CalibrationResult {
    /// The evaluated weights configuration.
    pub weights: RankingWeights,
    /// Mean nDCG@5 score.
    pub mean_ndcg_at_5: f64,
    /// Mean nDCG@10 score.
    pub mean_ndcg_at_10: f64,
    /// Mean MRR score.
    pub mean_mrr: f64,
    /// Mean Recall@5 score.
    pub mean_recall_at_5: f64,
    /// Mean Precision@5 score.
    pub mean_precision_at_5: f64,
}

/// In-memory representation of a query's candidates and context.
pub struct QueryEvaluationCache {
    /// The query identifier.
    pub query_id: String,
    /// Expected matching Node IDs.
    pub expected_node_ids: Vec<NodeId>,
    /// Acceptable alternative Node IDs.
    pub acceptable_alternatives: Vec<NodeId>,
    /// Retrieved candidates with their immutable database context.
    pub candidates: Vec<(RetrievalResult, FeatureContext)>,
}

/// An in-memory evaluation session containing cached feature vectors and ground truths.
pub struct EvaluationSession {
    /// Reference time point used to calculate age deltas.
    pub reference_time: u64,
    /// Exponential decay parameters.
    pub decay: crate::retrieval::eval_harness::RankingDecay,
    /// Cached query data.
    pub cache: Vec<QueryEvaluationCache>,
}

impl EvaluationSession {
    /// Builds a new EvaluationSession by fetching and caching metadata from database.
    pub fn build<R: Retriever>(
        queries: &QueryCorpus,
        truth: &GroundTruthCorpus,
        retriever: &R,
        provider: &crate::retrieval::eval_harness::FeatureProvider,
        reference_time: u64,
        decay: crate::retrieval::eval_harness::RankingDecay,
    ) -> Result<Self, BrainError> {
        let mut cache = Vec::with_capacity(queries.queries.len());

        for query in &queries.queries {
            let truth_item = match truth.ground_truth.get(&query.query_id) {
                Some(t) => t,
                None => continue,
            };

            let expected_node_ids: Vec<NodeId> = truth_item
                .expected_node_ids
                .iter()
                .map(|s| {
                    uuid::Uuid::parse_str(s)
                        .map(NodeId)
                        .map_err(|e| BrainError::Internal { message: format!("Invalid UUID: {}", e) })
                })
                .collect::<Result<Vec<NodeId>, BrainError>>()?;

            let acceptable_alternatives: Vec<NodeId> = truth_item
                .acceptable_alternatives
                .iter()
                .map(|s| {
                    uuid::Uuid::parse_str(s)
                        .map(NodeId)
                        .map_err(|e| BrainError::Internal { message: format!("Invalid UUID: {}", e) })
                })
                .collect::<Result<Vec<NodeId>, BrainError>>()?;

            let results = retriever.retrieve(&query.text)?;
            if results.is_empty() {
                cache.push(QueryEvaluationCache {
                    query_id: query.query_id.clone(),
                    expected_node_ids,
                    acceptable_alternatives,
                    candidates: vec![],
                });
                continue;
            }

            let node_ids: Vec<NodeId> = results.iter().map(|r| r.node_id).collect();
            let contexts = provider.load_contexts(&node_ids)?;

            let mut candidates = Vec::with_capacity(results.len());
            for res in results {
                let default_ctx = FeatureContext {
                    updated_at: None,
                    importance: None,
                    pinned: false,
                    provenance_confidence: None,
                    graph_degree: None,
                    access_count: None,
                    last_observed_at: None,
                };
                let context = contexts.get(&res.node_id).cloned().unwrap_or(default_ctx);
                candidates.push((res, context));
            }

            cache.push(QueryEvaluationCache {
                query_id: query.query_id.clone(),
                expected_node_ids,
                acceptable_alternatives,
                candidates,
            });
        }

        Ok(Self {
            reference_time,
            decay,
            cache,
        })
    }

    /// Evaluates in-memory candidate cache against specific weights.
    pub fn evaluate(&self, weights: RankingWeights) -> CalibrationResult {
        let ranker = LinearRanker::new(weights);
        let extractor = FeatureExtractor::new(self.reference_time, self.decay);

        let mut sum_recall_at_5 = 0.0;
        let mut sum_precision_at_5 = 0.0;
        let mut sum_mrr = 0.0;
        let mut sum_ndcg_at_5 = 0.0;
        let mut sum_ndcg_at_10 = 0.0;
        let mut success_count = 0;

        for query_cache in &self.cache {
            if query_cache.candidates.is_empty() {
                continue;
            }

            let mut scored_results = Vec::with_capacity(query_cache.candidates.len());
            for (res, ctx) in &query_cache.candidates {
                let features = extractor.extract(res, ctx);
                let score = ranker.score(&features);
                let mut cloned_res = res.clone();
                cloned_res.ranking_score = Some(score);
                scored_results.push(cloned_res);
            }

            crate::retrieval::eval_harness::sort_results_deterministically(&mut scored_results);
            let retrieved_ids: Vec<NodeId> = scored_results.iter().map(|r| r.node_id).collect();

            let recall_at_5 = compute_recall_at_k(&retrieved_ids, &query_cache.expected_node_ids, 5);
            let precision_at_5 = compute_precision_at_k(
                &retrieved_ids,
                &query_cache.expected_node_ids,
                &query_cache.acceptable_alternatives,
                5,
            );
            let mrr = compute_mrr(
                &retrieved_ids,
                &query_cache.expected_node_ids,
                &query_cache.acceptable_alternatives,
            );
            let ndcg_at_5 = compute_ndcg_at_k(
                &retrieved_ids,
                &query_cache.expected_node_ids,
                &query_cache.acceptable_alternatives,
                5,
            );
            let ndcg_at_10 = compute_ndcg_at_k(
                &retrieved_ids,
                &query_cache.expected_node_ids,
                &query_cache.acceptable_alternatives,
                10,
            );

            sum_recall_at_5 += recall_at_5;
            sum_precision_at_5 += precision_at_5;
            sum_mrr += mrr;
            sum_ndcg_at_5 += ndcg_at_5;
            sum_ndcg_at_10 += ndcg_at_10;
            success_count += 1;
        }

        let divisor = if success_count > 0 { success_count as f64 } else { 1.0 };

        CalibrationResult {
            weights,
            mean_ndcg_at_5: sum_ndcg_at_5 / divisor,
            mean_ndcg_at_10: sum_ndcg_at_10 / divisor,
            mean_mrr: sum_mrr / divisor,
            mean_recall_at_5: sum_recall_at_5 / divisor,
            mean_precision_at_5: sum_precision_at_5 / divisor,
        }
    }
}

/// The calibration engine coordinating the evaluation options and returning optimization lists.
pub struct CalibrationEngine;

impl CalibrationEngine {
    /// Executes the calibration search over weight parameters and returns results sorted by target objective.
    pub fn run_calibration(
        session: &EvaluationSession,
        options: CalibrationOptions,
        objective: CalibrationObjective,
    ) -> Vec<CalibrationResult> {
        let weights_list = match options {
            CalibrationOptions::Grid {
                lexical_weights,
                semantic_weights,
                recency_weights,
                importance_weights,
                provenance_weights,
                graph_degree_weights,
                access_frequency_weights,
                freshness_decay_weights,
            } => {
                let product = cartesian_product(&[
                    lexical_weights,
                    semantic_weights,
                    recency_weights,
                    importance_weights,
                    provenance_weights,
                    graph_degree_weights,
                    access_frequency_weights,
                    freshness_decay_weights,
                ]);
                product
                    .into_iter()
                    .map(|v| RankingWeights {
                        lexical: v[0],
                        semantic: v[1],
                        recency: v[2],
                        importance: v[3],
                        provenance_confidence: v[4],
                        graph_degree: v[5],
                        access_frequency: v[6],
                        freshness_decay: v[7],
                    })
                    .collect()
            }
            CalibrationOptions::Candidates(list) => list,
        };

        let mut results = Vec::with_capacity(weights_list.len());
        for weights in weights_list {
            let res = session.evaluate(weights);
            results.push(res);
        }

        // Sort descending by objective function score
        results.sort_by(|a, b| {
            let score_a = objective.score(a);
            let score_b = objective.score(b);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }
}

fn cartesian_product(lists: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let mut result = vec![vec![]];
    for list in lists {
        let mut next = vec![];
        for r in result {
            for &item in list {
                let mut new_r = r.clone();
                new_r.push(item);
                next.push(new_r);
            }
        }
        result = next;
    }
    result
}

/// Renderer-independent Markdown report writer.
pub struct MarkdownReportWriter;

impl MarkdownReportWriter {
    /// Formats the calibration results into a detailed Markdown comparison table.
    pub fn write_report(results: &[CalibrationResult], objective: CalibrationObjective) -> String {
        let mut report = String::new();
        report.push_str("# Calibration Report\n\n");
        report.push_str(&format!("Sorted by objective: **{:?}**\n\n", objective));

        report.push_str("| Rank | Lexical | Semantic | Recency | Importance | Confidence | Degree | Frequency | Decay | nDCG@5 | nDCG@10 | MRR | Recall@5 | Precision@5 | Target Score |\n");
        report.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");

        for (idx, res) in results.iter().enumerate().take(50) {
            let target_score = objective.score(res);
            report.push_str(&format!(
                "| {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} |\n",
                idx + 1,
                res.weights.lexical,
                res.weights.semantic,
                res.weights.recency,
                res.weights.importance,
                res.weights.provenance_confidence,
                res.weights.graph_degree,
                res.weights.access_frequency,
                res.weights.freshness_decay,
                res.mean_ndcg_at_5,
                res.mean_ndcg_at_10,
                res.mean_mrr,
                res.mean_recall_at_5,
                res.mean_precision_at_5,
                target_score
            ));
        }

        if results.len() > 50 {
            report.push_str(&format!("\n*(Truncated {} configurations...)*\n", results.len() - 50));
        }

        report
    }
}
