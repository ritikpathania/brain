use crate::identifiers::NodeId;
use crate::retrieval::models::SnapshotVersion;
use crate::consolidation::MetricConstructionError;

macro_rules! define_metric_score {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
        pub struct $name(f64);

        impl $name {
            /// Creates a new validated metric score between 0.0 and 1.0.
            pub fn new(val: f64) -> Result<Self, MetricConstructionError> {
                if !val.is_finite() {
                    return Err(MetricConstructionError::NotFinite { val });
                }
                if val < 0.0 || val > 1.0 {
                    return Err(MetricConstructionError::OutOfRange { val, min: 0.0, max: 1.0 });
                }
                Ok(Self(val))
            }

            /// Accesses the underlying score.
            pub fn value(&self) -> f64 {
                self.0
            }
        }
    };
}

define_metric_score!(NdcgScore, "Normalized Discounted Cumulative Gain score scaled [0.0, 1.0].");
define_metric_score!(MrrScore, "Mean Reciprocal Rank score scaled [0.0, 1.0].");
define_metric_score!(RecallScore, "Recall score scaled [0.0, 1.0].");
define_metric_score!(PrecisionScore, "Precision score scaled [0.0, 1.0].");

/// Relevance label mapping query-node pairs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RelevanceJudgment {
    /// Target candidate node under evaluation.
    pub node_id: NodeId,
    /// Relevance score (0.0 for irrelevant, 1.0+ for relevant).
    pub score: f64,
}

/// Evaluation case containing query context and expected judgments.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvaluationTestCase {
    /// Search query context.
    pub query: String,
    /// Candidates available for ranking.
    pub candidates: Vec<crate::Node>,
    /// Associated temporal edges active during ranking.
    pub temporal_edges: Vec<crate::temporal::TemporalEdge>,
    /// Relevance judgments for the query.
    pub judgments: Vec<RelevanceJudgment>,
}

/// Immutable evaluation dataset package.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvaluationDataset {
    /// Unique name or ID of evaluation dataset.
    pub version: String,
    /// Test cases matching the dataset profile.
    pub cases: Vec<EvaluationTestCase>,
}
/// Holds all evaluated metric scores.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvaluationMetrics {
    /// NDCG at depth K.
    pub ndcg_k: NdcgScore,
    /// Mean Reciprocal Rank.
    pub mrr: MrrScore,
    /// Recall at depth K.
    pub recall_k: RecallScore,
    /// Precision at depth K.
    pub precision_k: PrecisionScore,
}

/// Comparison between candidate model and baseline model performance.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvaluationComparison {
    /// Baseline model performance metrics.
    pub baseline: EvaluationMetrics,
    /// Candidate model performance metrics.
    pub candidate: EvaluationMetrics,
    /// Difference in NDCG (candidate - baseline).
    pub ndcg_improvement: f64,
    /// Difference in MRR (candidate - baseline).
    pub mrr_improvement: f64,
}

/// Computes metrics over ranked nodes.
pub struct MetricCalculator;

impl MetricCalculator {
    /// Computes Precision@K.
    pub fn precision(ranked: &[NodeId], judgments: &[RelevanceJudgment], k: usize) -> f64 {
        if k == 0 || ranked.is_empty() {
            return 0.0;
        }
        let k_limit = std::cmp::min(k, ranked.len());
        let top_k = &ranked[..k_limit];
        let mut relevant_retrieved = 0;
        for &id in top_k {
            if judgments.iter().any(|j| j.node_id == id && j.score > 0.0) {
                relevant_retrieved += 1;
            }
        }
        relevant_retrieved as f64 / k as f64
    }

    /// Computes Recall@K.
    pub fn recall(ranked: &[NodeId], judgments: &[RelevanceJudgment], k: usize) -> f64 {
        let total_relevant = judgments.iter().filter(|j| j.score > 0.0).count();
        if total_relevant == 0 {
            return 1.0;
        }
        if ranked.is_empty() || k == 0 {
            return 0.0;
        }
        let k_limit = std::cmp::min(k, ranked.len());
        let top_k = &ranked[..k_limit];
        let mut relevant_retrieved = 0;
        for &id in top_k {
            if judgments.iter().any(|j| j.node_id == id && j.score > 0.0) {
                relevant_retrieved += 1;
            }
        }
        relevant_retrieved as f64 / total_relevant as f64
    }

    /// Computes Reciprocal Rank.
    pub fn reciprocal_rank(ranked: &[NodeId], judgments: &[RelevanceJudgment]) -> f64 {
        for (idx, &id) in ranked.iter().enumerate() {
            if judgments.iter().any(|j| j.node_id == id && j.score > 0.0) {
                return 1.0 / (idx + 1) as f64;
            }
        }
        0.0
    }

    /// Computes Discounted Cumulative Gain (DCG) at depth K.
    pub fn dcg(ranked: &[NodeId], judgments: &[RelevanceJudgment], k: usize) -> f64 {
        if ranked.is_empty() || k == 0 {
            return 0.0;
        }
        let k_limit = std::cmp::min(k, ranked.len());
        let top_k = &ranked[..k_limit];
        let mut score = 0.0;
        for (idx, &id) in top_k.iter().enumerate() {
            if let Some(j) = judgments.iter().find(|j| j.node_id == id) {
                score += (2.0f64.powf(j.score) - 1.0) / ((idx + 2) as f64).log2();
            }
        }
        score
    }

    /// Computes Ideal Discounted Cumulative Gain (IDCG) at depth K.
    pub fn idcg(judgments: &[RelevanceJudgment], k: usize) -> f64 {
        if judgments.is_empty() || k == 0 {
            return 0.0;
        }
        let mut sorted_relevances: Vec<f64> = judgments.iter().map(|j| j.score).collect();
        sorted_relevances.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let k_limit = std::cmp::min(k, sorted_relevances.len());
        let mut score = 0.0;
        for (idx, &rel) in sorted_relevances[..k_limit].iter().enumerate() {
            score += (2.0f64.powf(rel) - 1.0) / ((idx + 2) as f64).log2();
        }
        score
    }

    /// Computes Normalized Discounted Cumulative Gain (NDCG) at depth K.
    pub fn ndcg(ranked: &[NodeId], judgments: &[RelevanceJudgment], k: usize) -> f64 {
        let idcg_val = Self::idcg(judgments, k);
        if idcg_val == 0.0 {
            return 1.0;
        }
        Self::dcg(ranked, judgments, k) / idcg_val
    }

    /// Computes aggregate EvaluationMetrics by composing metric functions.
    pub fn compute_metrics(
        ranked: &[NodeId],
        judgments: &[RelevanceJudgment],
        k: usize,
    ) -> Result<EvaluationMetrics, MetricConstructionError> {
        let ndcg_val = Self::ndcg(ranked, judgments, k);
        let rr_val = Self::reciprocal_rank(ranked, judgments);
        let recall_val = Self::recall(ranked, judgments, k);
        let precision_val = Self::precision(ranked, judgments, k);

        Ok(EvaluationMetrics {
            ndcg_k: NdcgScore::new(ndcg_val)?,
            mrr: MrrScore::new(rr_val)?,
            recall_k: RecallScore::new(recall_val)?,
            precision_k: PrecisionScore::new(precision_val)?,
        })
    }
}

/// Recommendation decision for candidate publication.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PublicationRecommendation {
    /// Performance is sufficient; approved.
    Approve,
    /// Performance degraded; rejected.
    Reject {
        /// Reason for rejection.
        reason: String,
    },
}

/// Trait defining candidate publication validation strategies.
pub trait PublicationPolicy: Send + Sync {
    /// Evaluates comparison metrics and yields a recommendation decision.
    fn evaluate_recommendation(&self, comparison: &EvaluationComparison) -> PublicationRecommendation;
    /// Returns the policy identifier.
    fn name(&self) -> &'static str;
}

/// Policy approving candidates as long as NDCG does not degrade.
#[derive(Clone, Copy)]
pub struct NoRegressionPolicy;

impl PublicationPolicy for NoRegressionPolicy {
    fn evaluate_recommendation(&self, comparison: &EvaluationComparison) -> PublicationRecommendation {
        if comparison.ndcg_improvement >= 0.0 {
            PublicationRecommendation::Approve
        } else {
            PublicationRecommendation::Reject {
                reason: format!("Candidate NDCG degraded by {:.4}", -comparison.ndcg_improvement),
            }
        }
    }

    fn name(&self) -> &'static str {
        "NoRegressionPolicy"
    }
}

/// Metadata defining parameters used during evaluation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvaluationMetadata {
    /// Dataset version used.
    pub dataset_version: String,
    /// Evaluation timestamp.
    pub timestamp: u64,
    /// Depth cutoff parameter.
    pub k: usize,
    /// Normalizer strategy description.
    pub normalizer_strategy: String,
    /// Publication policy label.
    pub publication_policy: String,
}

/// Audit-grade evaluation comparison summary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvaluationReport {
    /// Candidate weight snapshot version.
    pub candidate_version: SnapshotVersion,
    /// Baseline weight snapshot version.
    pub baseline_version: SnapshotVersion,
    /// Metric comparison details.
    pub comparison: EvaluationComparison,
    /// Recommendation outcome.
    pub recommendation: PublicationRecommendation,
    /// Context metadata configurations.
    pub metadata: EvaluationMetadata,
}
