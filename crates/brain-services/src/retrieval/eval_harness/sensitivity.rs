use crate::retrieval::eval_harness::{
    EvaluationSession, FeatureExtractor, FeatureVector, LinearRanker, RankingWeights,
};
use brain_domain::NodeId;
use std::collections::HashMap;

/// Detailed diagnostic impact metrics for a single feature.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeatureImpact {
    /// The canonical feature field name.
    pub feature_name: String,
    /// Minimum value observed in the corpus.
    pub min: f64,
    /// Maximum value observed in the corpus.
    pub max: f64,
    /// Mean value observed in the corpus.
    pub mean: f64,
    /// Standard deviation of the values.
    pub std_dev: f64,
    /// Average percentage numerical contribution of this feature to the final score.
    pub average_contribution_pct: f64,
    /// Average absolute rank position shift per candidate query list when this feature is removed.
    pub avg_rank_shift: f64,
    /// Number of queries whose ranking order changed when this feature is removed.
    pub queries_changed: usize,
    /// Percentage of total candidates across all queries that shifted rank positions when this feature is removed.
    pub affected_candidate_pct: f64,
    /// True if the standard deviation is below 1e-6.
    pub zero_variance: bool,
}

/// Sensitivity and feature diagnostics report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SensitivityReport {
    /// Total number of queries evaluated.
    pub total_queries: usize,
    /// Total number of candidate nodes evaluated.
    pub total_candidates: usize,
    /// Diagnostics metrics sorted alphabetically by feature name.
    pub impacts: Vec<FeatureImpact>,
}

/// Evaluates feature value distributions and rank influences on the session.
pub fn run_sensitivity_analysis(
    session: &EvaluationSession,
    baseline: RankingWeights,
) -> SensitivityReport {
    let extractor = FeatureExtractor::new(session.reference_time, session.decay);
    let ranker = LinearRanker::new(baseline);

    let feature_names = vec![
        "lexical_similarity".to_string(),
        "semantic_similarity".to_string(),
        "recency".to_string(),
        "importance".to_string(),
        "provenance_confidence".to_string(),
        "graph_degree".to_string(),
        "access_frequency".to_string(),
        "freshness_decay".to_string(),
    ];

    // 1. Gather all feature vectors, node IDs, and compute baseline scores
    struct QueryData {
        candidates: Vec<(NodeId, FeatureVector, f64)>, // (id, vector, baseline_score)
    }

    let mut processed_queries = Vec::with_capacity(session.cache.len());
    let mut total_candidates = 0;

    for query_cache in &session.cache {
        if query_cache.candidates.is_empty() {
            continue;
        }

        let mut candidates = Vec::with_capacity(query_cache.candidates.len());
        for (res, ctx) in &query_cache.candidates {
            let vector = extractor.extract(res, ctx);
            let score = ranker.score(&vector);
            candidates.push((res.node_id, vector, score));
        }

        total_candidates += candidates.len();

        processed_queries.push(QueryData {
            candidates,
        });
    }

    let mut impacts = Vec::with_capacity(feature_names.len());

    for name in &feature_names {
        // Collect all feature values across the entire corpus
        let mut values = Vec::new();
        for q in &processed_queries {
            for (_, vector, _) in &q.candidates {
                values.push(get_feature_value(vector, name));
            }
        }

        // Calculate distribution stats
        let count = values.len() as f64;
        let mut min = 0.0;
        let mut max = 0.0;
        let mut mean = 0.0;
        let mut std_dev = 0.0;
        let mut zero_variance = true;

        if !values.is_empty() {
            min = values[0];
            max = values[0];
            let mut sum = 0.0;
            for &v in &values {
                if v < min {
                    min = v;
                }
                if v > max {
                    max = v;
                }
                sum += v;
            }
            mean = sum / count;

            let mut var_sum = 0.0;
            for &v in &values {
                var_sum += (v - mean) * (v - mean);
            }
            let variance = var_sum / count;
            std_dev = variance.sqrt();
            zero_variance = std_dev < 1e-6;
        }

        // Calculate average contribution
        let mut sum_contrib_pct = 0.0;
        let mut contrib_count = 0;
        for q in &processed_queries {
            for (_, vector, score) in &q.candidates {
                let v = get_feature_value(vector, name);
                let w = get_feature_weight(baseline, name);
                let contrib = v * w;
                if *score > 0.0 {
                    sum_contrib_pct += contrib / *score;
                    contrib_count += 1;
                }
            }
        }
        let average_contribution_pct = if contrib_count > 0 {
            sum_contrib_pct / (contrib_count as f64)
        } else {
            0.0
        };

        // Ablation rankings comparison
        let modified_weights = disable_feature_weight(baseline, name);
        let modified_ranker = LinearRanker::new(modified_weights);

        let mut sum_rank_shifts = 0.0;
        let mut queries_changed = 0;
        let mut shifted_candidates = 0;

        for q in &processed_queries {
            // Sort under baseline weights
            let mut baseline_list = Vec::with_capacity(q.candidates.len());
            for (node_id, _, score) in &q.candidates {
                let res = crate::retrieval::eval_harness::RetrievalResult {
                    node_id: *node_id,
                    channel_scores: HashMap::new(),
                    ranking_score: Some(*score),
                };
                baseline_list.push(res);
            }
            crate::retrieval::eval_harness::sort_results_deterministically(&mut baseline_list);
            let baseline_order: Vec<NodeId> = baseline_list.iter().map(|r| r.node_id).collect();

            // Sort under modified weights
            let mut modified_list = Vec::with_capacity(q.candidates.len());
            for (node_id, vector, _) in &q.candidates {
                let score = modified_ranker.score(vector);
                let res = crate::retrieval::eval_harness::RetrievalResult {
                    node_id: *node_id,
                    channel_scores: HashMap::new(),
                    ranking_score: Some(score),
                };
                modified_list.push(res);
            }
            crate::retrieval::eval_harness::sort_results_deterministically(&mut modified_list);
            let modified_order: Vec<NodeId> = modified_list.iter().map(|r| r.node_id).collect();

            // Compare positions
            let mut query_changed = false;
            let mut query_shifts = 0.0;
            for (baseline_idx, &node_id) in baseline_order.iter().enumerate() {
                let modified_idx = modified_order
                    .iter()
                    .position(|&id| id == node_id)
                    .unwrap_or(baseline_idx);

                let shift = (baseline_idx as isize - modified_idx as isize).abs() as f64;
                query_shifts += shift;

                if shift > 0.0 {
                    query_changed = true;
                    shifted_candidates += 1;
                }
            }

            sum_rank_shifts += query_shifts / (q.candidates.len() as f64);
            if query_changed {
                queries_changed += 1;
            }
        }

        let divisor = if !processed_queries.is_empty() {
            processed_queries.len() as f64
        } else {
            1.0
        };

        let avg_rank_shift = sum_rank_shifts / divisor;
        let affected_candidate_pct = if total_candidates > 0 {
            (shifted_candidates as f64) / (total_candidates as f64)
        } else {
            0.0
        };

        impacts.push(FeatureImpact {
            feature_name: name.clone(),
            min,
            max,
            mean,
            std_dev,
            average_contribution_pct,
            avg_rank_shift,
            queries_changed,
            affected_candidate_pct,
            zero_variance,
        });
    }

    // Sort impacts alphabetically by feature name
    impacts.sort_by(|a, b| a.feature_name.cmp(&b.feature_name));

    SensitivityReport {
        total_queries: processed_queries.len(),
        total_candidates,
        impacts,
    }
}

fn get_feature_value(vector: &FeatureVector, name: &str) -> f64 {
    match name {
        "lexical_similarity" => vector.lexical_similarity.unwrap_or(0.0),
        "semantic_similarity" => vector.semantic_similarity.unwrap_or(0.0),
        "recency" => vector.recency.unwrap_or(0.0),
        "importance" => vector.importance.unwrap_or(0.0),
        "provenance_confidence" => vector.provenance_confidence.unwrap_or(0.0),
        "graph_degree" => vector.graph_degree.unwrap_or(0.0),
        "access_frequency" => vector.access_frequency.unwrap_or(0.0),
        "freshness_decay" => vector.freshness_decay.unwrap_or(0.0),
        _ => 0.0,
    }
}

fn get_feature_weight(weights: RankingWeights, name: &str) -> f64 {
    match name {
        "lexical_similarity" => weights.lexical,
        "semantic_similarity" => weights.semantic,
        "recency" => weights.recency,
        "importance" => weights.importance,
        "provenance_confidence" => weights.provenance_confidence,
        "graph_degree" => weights.graph_degree,
        "access_frequency" => weights.access_frequency,
        "freshness_decay" => weights.freshness_decay,
        _ => 0.0,
    }
}

fn disable_feature_weight(weights: RankingWeights, name: &str) -> RankingWeights {
    let mut modified = weights;
    match name {
        "lexical_similarity" => modified.lexical = 0.0,
        "semantic_similarity" => modified.semantic = 0.0,
        "recency" => modified.recency = 0.0,
        "importance" => modified.importance = 0.0,
        "provenance_confidence" => modified.provenance_confidence = 0.0,
        "graph_degree" => modified.graph_degree = 0.0,
        "access_frequency" => modified.access_frequency = 0.0,
        "freshness_decay" => modified.freshness_decay = 0.0,
        _ => {}
    }
    modified
}

/// Renderer-independent report formatter.
pub struct SensitivityReportWriter;

impl SensitivityReportWriter {
    /// Renders the SensitivityReport into a diff-friendly Markdown document.
    pub fn write_report(report: &SensitivityReport) -> String {
        let mut md = String::new();
        md.push_str("# Feature Impact & Sensitivity Report\n\n");
        md.push_str(&format!(
            "Evaluated **{}** queries containing a total of **{}** candidates.\n\n",
            report.total_queries, report.total_candidates
        ));

        md.push_str("## Feature Invariants & Ablation Metrics\n\n");
        md.push_str("| Feature | Mean | Std Dev | Min | Max | Contribution | Avg Rank Shift | Queries Changed | Candidates Shifted % | Status |\n");
        md.push_str("| :--- | ---: | ---: | ---: | ---: | ---: | ---: | :---: | ---: | :---: |\n");

        for imp in &report.impacts {
            let status = if imp.zero_variance {
                "⚠️ Zero Variance"
            } else {
                "✓ Active"
            };

            md.push_str(&format!(
                "| {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.2}% | {:.4} | {}/{} | {:.1}% | {} |\n",
                imp.feature_name,
                imp.mean,
                imp.std_dev,
                imp.min,
                imp.max,
                imp.average_contribution_pct * 100.0,
                imp.avg_rank_shift,
                imp.queries_changed,
                report.total_queries,
                imp.affected_candidate_pct * 100.0,
                status
            ));
        }

        md.push_str("\n## Zero Variance Features Summary\n\n");
        let zero_vars: Vec<String> = report
            .impacts
            .iter()
            .filter(|i| i.zero_variance)
            .map(|i| format!("- `{}`", i.feature_name))
            .collect();

        if zero_vars.is_empty() {
            md.push_str("None. All features show non-zero variance across the candidate set.\n");
        } else {
            md.push_str("The following features are dormant (zero variance) and candidates for corpus enrichment:\n");
            md.push_str(&zero_vars.join("\n"));
            md.push_str("\n");
        }

        md
    }
}
