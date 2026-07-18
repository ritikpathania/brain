use crate::retrieval::feature_extractor::FeatureExtractor;
use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::RetrievalRequest;
use brain_domain::retrieval::evaluation::{
    EvaluationComparison, EvaluationDataset, EvaluationMetadata, EvaluationMetrics,
    EvaluationReport, MetricCalculator, MrrScore, NdcgScore, PrecisionScore, PublicationPolicy,
    RecallScore,
};
use brain_domain::retrieval::features::{FeatureNormalizer, NormalizationContext};
use brain_domain::retrieval::models::{LinearRankingModel, RankingModel, WeightSnapshot};
use std::sync::Arc;

/// Bundles dependencies and context for running evaluations.
pub struct EvaluationContext<'a> {
    /// Target dataset to evaluate on.
    pub dataset: &'a EvaluationDataset,
    /// Depth parameter K.
    pub k: usize,
    /// Normalizer strategy description.
    pub normalizer_strategy: NormalizationContext,
    /// Publication policy.
    pub publication_policy: &'a dyn PublicationPolicy,
    /// Source repositories context.
    pub repos: &'a dyn RepositorySet,
    /// Clock provider for timestamps.
    pub clock: &'a dyn brain_domain::temporal::Clock,
}

/// Service orchestrating offline evaluation of weight snapshots.
pub struct OfflineEvaluator {
    extractor: Arc<dyn FeatureExtractor>,
    normalizer: Arc<dyn FeatureNormalizer>,
}

impl OfflineEvaluator {
    /// Creates a new `OfflineEvaluator`.
    pub fn new(
        extractor: Arc<dyn FeatureExtractor>,
        normalizer: Arc<dyn FeatureNormalizer>,
    ) -> Self {
        Self {
            extractor,
            normalizer,
        }
    }

    /// Executes offline evaluations on the given context.
    pub fn evaluate(
        &self,
        candidate: &WeightSnapshot,
        baseline: &WeightSnapshot,
        context: &EvaluationContext,
    ) -> Result<EvaluationReport, BrainError> {
        let mut baseline_ndcg = 0.0;
        let mut baseline_mrr = 0.0;
        let mut baseline_recall = 0.0;
        let mut baseline_precision = 0.0;

        let mut candidate_ndcg = 0.0;
        let mut candidate_mrr = 0.0;
        let mut candidate_recall = 0.0;
        let mut candidate_precision = 0.0;

        let mut evaluated_cases = 0;
        let baseline_model = LinearRankingModel::new(baseline.weights.clone());
        let candidate_model = LinearRankingModel::new(candidate.weights.clone());

        for case in &context.dataset.cases {
            if case.candidates.is_empty() {
                continue;
            }

            let request = RetrievalRequest {
                session_id: brain_domain::SessionId::new(),
                query: case.query.clone(),
                limit: case.candidates.len(),
                exclude_ids: std::collections::HashSet::new(),
                deadline: None,
            };

            let raw = self.extractor.extract(
                &request,
                &case.candidates,
                &case.temporal_edges,
                context.repos,
            )?;
            let norm = self
                .normalizer
                .normalize(&raw, &context.normalizer_strategy)
                .map_err(|e| BrainError::Internal {
                    message: format!("{:?}", e),
                })?;

            let b_ranked = self.rank_candidates(&case.candidates, &norm, &baseline_model);
            let c_ranked = self.rank_candidates(&case.candidates, &norm, &candidate_model);

            let b_met = MetricCalculator::compute_metrics(&b_ranked, &case.judgments, context.k)
                .map_err(|e| BrainError::Internal {
                    message: format!("{:?}", e),
                })?;
            let c_met = MetricCalculator::compute_metrics(&c_ranked, &case.judgments, context.k)
                .map_err(|e| BrainError::Internal {
                    message: format!("{:?}", e),
                })?;

            baseline_ndcg += b_met.ndcg_k.value();
            baseline_mrr += b_met.mrr.value();
            baseline_recall += b_met.recall_k.value();
            baseline_precision += b_met.precision_k.value();

            candidate_ndcg += c_met.ndcg_k.value();
            candidate_mrr += c_met.mrr.value();
            candidate_recall += c_met.recall_k.value();
            candidate_precision += c_met.precision_k.value();

            evaluated_cases += 1;
        }

        let eval_count = if evaluated_cases > 0 {
            evaluated_cases as f64
        } else {
            1.0
        };

        let baseline_metrics = EvaluationMetrics {
            ndcg_k: NdcgScore::new(baseline_ndcg / eval_count).map_err(|e| {
                BrainError::Internal {
                    message: format!("{:?}", e),
                }
            })?,
            mrr: MrrScore::new(baseline_mrr / eval_count).map_err(|e| BrainError::Internal {
                message: format!("{:?}", e),
            })?,
            recall_k: RecallScore::new(baseline_recall / eval_count).map_err(|e| {
                BrainError::Internal {
                    message: format!("{:?}", e),
                }
            })?,
            precision_k: PrecisionScore::new(baseline_precision / eval_count).map_err(|e| {
                BrainError::Internal {
                    message: format!("{:?}", e),
                }
            })?,
        };

        let candidate_metrics = EvaluationMetrics {
            ndcg_k: NdcgScore::new(candidate_ndcg / eval_count).map_err(|e| {
                BrainError::Internal {
                    message: format!("{:?}", e),
                }
            })?,
            mrr: MrrScore::new(candidate_mrr / eval_count).map_err(|e| BrainError::Internal {
                message: format!("{:?}", e),
            })?,
            recall_k: RecallScore::new(candidate_recall / eval_count).map_err(|e| {
                BrainError::Internal {
                    message: format!("{:?}", e),
                }
            })?,
            precision_k: PrecisionScore::new(candidate_precision / eval_count).map_err(|e| {
                BrainError::Internal {
                    message: format!("{:?}", e),
                }
            })?,
        };

        let ndcg_improvement = candidate_metrics.ndcg_k.value() - baseline_metrics.ndcg_k.value();
        let mrr_improvement = candidate_metrics.mrr.value() - baseline_metrics.mrr.value();

        let comparison = EvaluationComparison {
            baseline: baseline_metrics,
            candidate: candidate_metrics,
            ndcg_improvement,
            mrr_improvement,
        };

        let recommendation = context
            .publication_policy
            .evaluate_recommendation(&comparison);

        let metadata = EvaluationMetadata {
            dataset_version: context.dataset.version.clone(),
            timestamp: context.clock.now().unix_seconds(),
            k: context.k,
            normalizer_strategy: format!("{:?}", context.normalizer_strategy),
            publication_policy: context.publication_policy.name().to_string(),
        };

        Ok(EvaluationReport {
            candidate_version: candidate.metadata.version,
            baseline_version: baseline.metadata.version,
            comparison,
            recommendation,
            metadata,
        })
    }

    fn rank_candidates(
        &self,
        candidates: &[brain_domain::Node],
        signals: &[brain_domain::retrieval::models::RankingSignals],
        model: &LinearRankingModel,
    ) -> Vec<brain_domain::NodeId> {
        let mut scored: Vec<(brain_domain::NodeId, f64)> = candidates
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                let score = model.score(&signals[idx]);
                (node.id, score)
            })
            .collect();

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0 .0.cmp(&b.0 .0))
        });

        scored.into_iter().map(|(id, _)| id).collect()
    }
}
