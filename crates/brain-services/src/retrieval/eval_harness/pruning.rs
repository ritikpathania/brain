use crate::retrieval::eval_harness::{
    CalibrationEngine, CalibrationObjective, CalibrationOptions, EvaluationSession, Feature,
    RankingWeights,
};
use brain_core::errors::BrainError;

/// Classification of quality degradation impact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DegradationImpact {
    /// Negligible or zero impact on quality (delta >= -0.001).
    SafeToPrune,
    /// Minor quality degradation (-0.01 <= delta < -0.001).
    Minor,
    /// Moderate quality degradation (-0.05 <= delta < -0.01).
    Moderate,
    /// Critical quality degradation (delta < -0.05).
    Critical,
}

impl DegradationImpact {
    /// Classifies the degradation based on composite score delta.
    pub fn classify(delta: f64) -> Self {
        if delta >= -0.001 {
            Self::SafeToPrune
        } else if delta >= -0.01 {
            Self::Minor
        } else if delta >= -0.05 {
            Self::Moderate
        } else {
            Self::Critical
        }
    }

    /// Returns human-readable label string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SafeToPrune => "🟢 Safe to Prune",
            Self::Minor => "🟡 Minor Impact",
            Self::Moderate => "🟠 Moderate Impact",
            Self::Critical => "🔴 Critical",
        }
    }
}

/// Structured failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PruningFailureReason {
    /// Recalibration failed to execute.
    CalibrationFailed,
    /// No configuration met minimal validation requirements.
    NoValidConfiguration,
    /// Search space was empty or invalid.
    InvalidCalibrationSpace,
}

impl PruningFailureReason {
    /// Returns human-readable explanation of failure.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CalibrationFailed => "Calibration Failed",
            Self::NoValidConfiguration => "No Valid Configuration",
            Self::InvalidCalibrationSpace => "Invalid Calibration Space",
        }
    }
}

/// Structured outcome of a single-feature pruning experiment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PruningOutcome {
    /// Success containing the detailed ablated recalibration result.
    Success(PruningResult),
    /// Failure with failure metadata.
    Failure {
        /// The feature that was pruned.
        feature: Feature,
        /// Categorized failure reason.
        reason: PruningFailureReason,
        /// Verbose diagnostic error message.
        message: String,
    },
}

impl PruningOutcome {
    /// Helper to get the target pruned feature.
    pub fn feature(&self) -> Feature {
        match self {
            Self::Success(r) => r.pruned_feature,
            Self::Failure { feature, .. } => *feature,
        }
    }

    /// Helper to get composite degradation score (returns positive value representing loss, or 0 for failures).
    pub fn loss(&self) -> f64 {
        match self {
            Self::Success(r) => -r.composite_delta,
            Self::Failure { .. } => -9999.0, // Sort failures at the bottom/top depending on priority
        }
    }
}

/// Detailed metrics calculated after optimal retraining without the target feature.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PruningResult {
    /// The feature that was ablated/pruned.
    pub pruned_feature: Feature,
    /// The new optimal weights found after grid recalibration.
    pub optimal_weights: RankingWeights,
    /// Calibrated composite score without the feature.
    pub composite_score: f64,
    /// Absolute change in composite score compared to baseline.
    pub composite_delta: f64,
    /// nDCG@5 score after retraining.
    pub ndcg_at_5: f64,
    /// absolute change in nDCG@5 compared to baseline.
    pub ndcg_at_5_delta: f64,
    /// MRR score after retraining.
    pub mrr: f64,
    /// absolute change in MRR compared to baseline.
    pub mrr_delta: f64,
    /// Recall@5 score after retraining.
    pub recall_at_5: f64,
    /// absolute change in Recall@5 compared to baseline.
    pub recall_at_5_delta: f64,
    /// Number of distinct parameter candidates evaluated.
    pub calibration_candidates_tested: usize,
    /// Categorized degradation impact assessment.
    pub impact: DegradationImpact,
    /// The baseline weight of this feature before ablation.
    pub baseline_weight: f64,
}

/// The aggregated comparative pruning report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PruningExperimentReport {
    /// Calibration weights of the original baseline.
    pub baseline_weights: RankingWeights,
    /// Baseline composite score.
    pub baseline_composite: f64,
    /// Baseline nDCG@5 score.
    pub baseline_ndcg_at_5: f64,
    /// Baseline MRR score.
    pub baseline_mrr: f64,
    /// Baseline Recall@5 score.
    pub baseline_recall_at_5: f64,
    /// Outcomes of each pruning pass sorted by degradation descending.
    pub outcomes: Vec<PruningOutcome>,
    /// Calibration objective used to direct search.
    pub objective: CalibrationObjective,
}

/// Feature pruning experiment runner.
pub struct PruningExperimentRunner;

impl PruningExperimentRunner {
    /// Runs optimal retraining for each feature disabled to evaluate quality degradation.
    pub fn run_experiment(
        session: &EvaluationSession,
        options: &CalibrationOptions,
        objective: CalibrationObjective,
        prune_list: &[Feature],
    ) -> Result<PruningExperimentReport, BrainError> {
        // 1. Run baseline calibration
        let baseline_candidates =
            CalibrationEngine::run_calibration(session, options.clone(), objective);
        let baseline_opt = baseline_candidates
            .first()
            .ok_or_else(|| BrainError::Validation {
                message: "Baseline calibration produced no valid results.".to_string(),
            })?;

        let baseline_composite = objective.score(baseline_opt);
        let baseline_ndcg_at_5 = baseline_opt.mean_ndcg_at_5;
        let baseline_mrr = baseline_opt.mean_mrr;
        let baseline_recall_at_5 = baseline_opt.mean_recall_at_5;

        let mut outcomes = Vec::with_capacity(prune_list.len());

        // 2. Perform ablation grid calibrations
        for &feature in prune_list {
            let ablated_options = disable_feature_in_grid(options, feature);
            let candidates_tested = count_candidates(&ablated_options);

            if candidates_tested == 0 {
                outcomes.push(PruningOutcome::Failure {
                    feature,
                    reason: PruningFailureReason::InvalidCalibrationSpace,
                    message: "Calibration space has 0 candidates after pruning.".to_string(),
                });
                continue;
            }

            let ablated_candidates =
                CalibrationEngine::run_calibration(session, ablated_options, objective);
            let ablated_opt = match ablated_candidates.first() {
                Some(opt) => opt,
                None => {
                    outcomes.push(PruningOutcome::Failure {
                        feature,
                        reason: PruningFailureReason::NoValidConfiguration,
                        message: "Grid search returned an empty candidate list.".to_string(),
                    });
                    continue;
                }
            };

            let composite_score = objective.score(ablated_opt);
            let composite_delta = composite_score - baseline_composite;
            let ndcg_at_5 = ablated_opt.mean_ndcg_at_5;
            let ndcg_at_5_delta = ndcg_at_5 - baseline_ndcg_at_5;
            let mrr = ablated_opt.mean_mrr;
            let mrr_delta = mrr - baseline_mrr;
            let recall_at_5 = ablated_opt.mean_recall_at_5;
            let recall_at_5_delta = recall_at_5 - baseline_recall_at_5;

            let impact = DegradationImpact::classify(composite_delta);

            let baseline_weight = match feature {
                Feature::AccessFrequency => baseline_opt.weights.access_frequency,
                Feature::FreshnessDecay => baseline_opt.weights.freshness_decay,
                Feature::GraphDegree => baseline_opt.weights.graph_degree,
                Feature::Importance => baseline_opt.weights.importance,
                Feature::LexicalSimilarity => baseline_opt.weights.lexical,
                Feature::ProvenanceConfidence => baseline_opt.weights.provenance_confidence,
                Feature::Recency => baseline_opt.weights.recency,
                Feature::SemanticSimilarity => baseline_opt.weights.semantic,
            };

            outcomes.push(PruningOutcome::Success(PruningResult {
                pruned_feature: feature,
                optimal_weights: ablated_opt.weights,
                composite_score,
                composite_delta,
                ndcg_at_5,
                ndcg_at_5_delta,
                mrr,
                mrr_delta,
                recall_at_5,
                recall_at_5_delta,
                calibration_candidates_tested: candidates_tested,
                impact,
                baseline_weight,
            }));
        }

        // 3. Sort outcomes descending by degradation (largest negative delta first)
        // Failure outcomes are pushed to the bottom.
        outcomes.sort_by(|a, b| {
            match (a, b) {
                (PruningOutcome::Success(ra), PruningOutcome::Success(rb)) => {
                    // Sort descending by degradation (i.e. ra.composite_delta ascending since negative delta is degradation)
                    ra.composite_delta
                        .partial_cmp(&rb.composite_delta)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
                (PruningOutcome::Success(_), PruningOutcome::Failure { .. }) => {
                    std::cmp::Ordering::Less
                }
                (PruningOutcome::Failure { .. }, PruningOutcome::Success(_)) => {
                    std::cmp::Ordering::Greater
                }
                (
                    PruningOutcome::Failure { feature: fa, .. },
                    PruningOutcome::Failure { feature: fb, .. },
                ) => {
                    // Tie-break alphabetically on feature
                    fa.cmp(fb)
                }
            }
        });

        Ok(PruningExperimentReport {
            baseline_weights: baseline_opt.weights,
            baseline_composite,
            baseline_ndcg_at_5,
            baseline_mrr,
            baseline_recall_at_5,
            outcomes,
            objective,
        })
    }
}

fn disable_feature_in_grid(options: &CalibrationOptions, feature: Feature) -> CalibrationOptions {
    match options {
        CalibrationOptions::Grid {
            lexical_weights,
            semantic_weights,
            recency_weights,
            importance_weights,
            provenance_weights,
            graph_degree_weights,
            access_frequency_weights,
            freshness_decay_weights,
        } => CalibrationOptions::Grid {
            lexical_weights: if feature == Feature::LexicalSimilarity {
                vec![0.0]
            } else {
                lexical_weights.clone()
            },
            semantic_weights: if feature == Feature::SemanticSimilarity {
                vec![0.0]
            } else {
                semantic_weights.clone()
            },
            recency_weights: if feature == Feature::Recency {
                vec![0.0]
            } else {
                recency_weights.clone()
            },
            importance_weights: if feature == Feature::Importance {
                vec![0.0]
            } else {
                importance_weights.clone()
            },
            provenance_weights: if feature == Feature::ProvenanceConfidence {
                vec![0.0]
            } else {
                provenance_weights.clone()
            },
            graph_degree_weights: if feature == Feature::GraphDegree {
                vec![0.0]
            } else {
                graph_degree_weights.clone()
            },
            access_frequency_weights: if feature == Feature::AccessFrequency {
                vec![0.0]
            } else {
                access_frequency_weights.clone()
            },
            freshness_decay_weights: if feature == Feature::FreshnessDecay {
                vec![0.0]
            } else {
                freshness_decay_weights.clone()
            },
        },
        CalibrationOptions::Candidates(list) => {
            let modified = list
                .iter()
                .map(|w| {
                    let mut mw = w.clone();
                    match feature {
                        Feature::LexicalSimilarity => mw.lexical = 0.0,
                        Feature::SemanticSimilarity => mw.semantic = 0.0,
                        Feature::Recency => mw.recency = 0.0,
                        Feature::Importance => mw.importance = 0.0,
                        Feature::ProvenanceConfidence => mw.provenance_confidence = 0.0,
                        Feature::GraphDegree => mw.graph_degree = 0.0,
                        Feature::AccessFrequency => mw.access_frequency = 0.0,
                        Feature::FreshnessDecay => mw.freshness_decay = 0.0,
                    }
                    mw
                })
                .collect();
            CalibrationOptions::Candidates(modified)
        }
    }
}

fn count_candidates(options: &CalibrationOptions) -> usize {
    match options {
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
            lexical_weights.len()
                * semantic_weights.len()
                * recency_weights.len()
                * importance_weights.len()
                * provenance_weights.len()
                * graph_degree_weights.len()
                * access_frequency_weights.len()
                * freshness_decay_weights.len()
        }
        CalibrationOptions::Candidates(list) => list.len(),
    }
}

/// Renderer formatting pruning experiment reports.
pub struct PruningReportWriter;

impl PruningReportWriter {
    /// Renders a comparative Markdown pruning experiment table.
    pub fn write_report(report: &PruningExperimentReport) -> String {
        let mut md = String::new();
        md.push_str("# Feature Pruning & Degradation Report\n\n");
        md.push_str("> [!IMPORTANT]\n");
        md.push_str("> Controlled benchmarks intentionally exaggerate feature influence to verify ranking behavior.\n");
        md.push_str("> Disabling a critical feature followed by optimal grid recalibration measures the maximum degradation risk.\n\n");

        md.push_str("## Baseline Calibrated Setup\n\n");
        md.push_str(&format!("- Objective Metric: **{:?}**\n", report.objective));
        md.push_str(&format!(
            "- Baseline Composite Score: **{:.4}** (nDCG@5: **{:.4}**, MRR: **{:.4}**, Recall@5: **{:.4}**)\n",
            report.baseline_composite, report.baseline_ndcg_at_5, report.baseline_mrr, report.baseline_recall_at_5
        ));
        md.push_str(&format!(
            "- Baseline Weights: `lexical={:.2}, semantic={:.2}, recency={:.2}, importance={:.2}, provenance={:.2}, graph={:.2}, access={:.2}, freshness={:.2}`\n\n",
            report.baseline_weights.lexical,
            report.baseline_weights.semantic,
            report.baseline_weights.recency,
            report.baseline_weights.importance,
            report.baseline_weights.provenance_confidence,
            report.baseline_weights.graph_degree,
            report.baseline_weights.access_frequency,
            report.baseline_weights.freshness_decay
        ));

        md.push_str("## Recalibrated Ablation Matrix\n\n");
        md.push_str("| Pruned Feature | Baseline Weight | Retrained Composite | Composite Δ | Retrained nDCG@5 | MRR | Recall@5 | Cost (Candidates) | Impact |\n");
        md.push_str("| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |\n");

        for outcome in &report.outcomes {
            match outcome {
                PruningOutcome::Success(res) => {
                    md.push_str(&format!(
                        "| **{}** | {:.2} | {:.4} | {:+.4} | {:.4} | {:.4} | {:.4} | {} | {} |\n",
                        res.pruned_feature.as_str(),
                        res.baseline_weight,
                        res.composite_score,
                        res.composite_delta,
                        res.ndcg_at_5,
                        res.mrr,
                        res.recall_at_5,
                        res.calibration_candidates_tested,
                        res.impact.as_str()
                    ));
                }
                PruningOutcome::Failure {
                    feature,
                    reason,
                    message,
                } => {
                    md.push_str(&format!(
                        "| **{}** | *N/A* | *N/A* | *N/A* | *N/A* | *N/A* | *N/A* | 0 | ❌ Failure ({}: {}) |\n",
                        feature.as_str(),
                        reason.as_str(),
                        message
                    ));
                }
            }
        }

        md.push_str("\n## Optimal Ablated Weight Profiles\n\n");
        for outcome in &report.outcomes {
            if let PruningOutcome::Success(res) = outcome {
                md.push_str(&format!(
                    "- **{}** disabled: `lexical={:.2}, semantic={:.2}, recency={:.2}, importance={:.2}, provenance={:.2}, graph={:.2}, access={:.2}, freshness={:.2}`\n",
                    res.pruned_feature.as_str(),
                    res.optimal_weights.lexical,
                    res.optimal_weights.semantic,
                    res.optimal_weights.recency,
                    res.optimal_weights.importance,
                    res.optimal_weights.provenance_confidence,
                    res.optimal_weights.graph_degree,
                    res.optimal_weights.access_frequency,
                    res.optimal_weights.freshness_decay
                ));
            }
        }

        md
    }
}
