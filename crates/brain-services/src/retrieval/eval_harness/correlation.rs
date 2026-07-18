#![allow(missing_docs)]
use crate::retrieval::eval_harness::{EvaluationSession, FeatureExtractor, FeatureVector};
use brain_core::errors::BrainError;

/// Enumeration of all 8 ranking features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Feature {
    /// Number of times the node was selected in query results.
    AccessFrequency,
    /// Temporal freshness decay multiplier.
    FreshnessDecay,
    /// Number of connected edges in long-term memory graph.
    GraphDegree,
    /// Static query importance/boost property.
    Importance,
    /// Lexical token search similarity.
    LexicalSimilarity,
    /// Provenance origin metadata confidence score.
    ProvenanceConfidence,
    /// Recency decay derived from node modification timestamp.
    Recency,
    /// Latent vector semantic similarity cosine.
    SemanticSimilarity,
}

impl Feature {
    /// Returns a vector of all features sorted alphabetically.
    pub fn all() -> Vec<Self> {
        vec![
            Self::AccessFrequency,
            Self::FreshnessDecay,
            Self::GraphDegree,
            Self::Importance,
            Self::LexicalSimilarity,
            Self::ProvenanceConfidence,
            Self::Recency,
            Self::SemanticSimilarity,
        ]
    }

    /// Returns the canonical field name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AccessFrequency => "access_frequency",
            Self::FreshnessDecay => "freshness_decay",
            Self::GraphDegree => "graph_degree",
            Self::Importance => "importance",
            Self::LexicalSimilarity => "lexical_similarity",
            Self::ProvenanceConfidence => "provenance_confidence",
            Self::Recency => "recency",
            Self::SemanticSimilarity => "semantic_similarity",
        }
    }
}

/// Correlation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CorrelationMethod {
    /// Linear correlation coefficient.
    Pearson,
    /// Rank-order monotonic correlation.
    Spearman,
}

/// Level of redundancy warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RedundancyLevel {
    /// Perfectly collinear or redundant signals (r >= 0.999).
    PerfectlyRedundant,
    /// Highly correlated signals (r >= threshold).
    HighlyCorrelated,
}

/// Redundancy warning for a pair of features.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RedundantFeaturePair {
    /// First feature in the pair.
    pub feature_a: Feature,
    /// Second feature in the pair.
    pub feature_b: Feature,
    /// Measured correlation coefficient.
    pub correlation: f64,
    /// Category/severity of redundancy.
    pub level: RedundancyLevel,
}

/// Decoupled matrix type representing correlation coefficients.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeatureCorrelationMatrix {
    /// List of features representing rows and columns.
    pub features: Vec<Feature>,
    /// 2D vector of correlation coefficients.
    pub values: Vec<Vec<f64>>,
}

/// Complete in-memory correlation report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CorrelationReport {
    /// Method utilized for the analysis.
    pub method: CorrelationMethod,
    /// Computed feature correlation matrix.
    pub matrix: FeatureCorrelationMatrix,
    /// Detected redundant pairs.
    pub redundant_pairs: Vec<RedundantFeaturePair>,
    /// Correlation coefficient threshold.
    pub threshold: f64,
    /// Total number of candidate nodes evaluated.
    pub total_candidates: usize,
}

/// Correlation analysis engine.
pub struct CorrelationEngine;

impl CorrelationEngine {
    /// Runs correlation analysis over the cached feature vectors in an EvaluationSession.
    pub fn run_analysis(
        session: &EvaluationSession,
        method: CorrelationMethod,
        threshold: f64,
    ) -> Result<CorrelationReport, BrainError> {
        let extractor = FeatureExtractor::new(session.reference_time, session.decay);

        // 1. Collect all candidates' FeatureVectors
        let mut vectors = Vec::new();
        for query_cache in &session.cache {
            for (res, ctx) in &query_cache.candidates {
                vectors.push(extractor.extract(res, ctx));
            }
        }

        let total_candidates = vectors.len();
        if total_candidates == 0 {
            return Err(BrainError::Validation {
                message: "Cannot run correlation analysis on an empty candidate set.".to_string(),
            });
        }

        let features = Feature::all();
        let num_features = features.len();

        // 2. Extract values for each feature
        let mut feature_values = vec![vec![0.0; total_candidates]; num_features];
        for (i, feature) in features.iter().enumerate() {
            for (j, vec) in vectors.iter().enumerate() {
                feature_values[i][j] = get_feature_value(vec, feature);
            }
        }

        // 3. Compute values matrix based on selected correlation method
        let matrix_values = match method {
            CorrelationMethod::Pearson => {
                let mut values = vec![vec![0.0; num_features]; num_features];
                for i in 0..num_features {
                    for j in i..num_features {
                        let r = pearson_correlation(&feature_values[i], &feature_values[j]);
                        values[i][j] = r;
                        values[j][i] = r;
                    }
                }
                values
            }
            CorrelationMethod::Spearman => {
                // Rank-transform the values of each feature first
                let ranked_features: Vec<Vec<f64>> = feature_values
                    .iter()
                    .map(|vals| compute_ranks(vals))
                    .collect();

                let mut values = vec![vec![0.0; num_features]; num_features];
                for i in 0..num_features {
                    for j in i..num_features {
                        let r = pearson_correlation(&ranked_features[i], &ranked_features[j]);
                        values[i][j] = r;
                        values[j][i] = r;
                    }
                }
                values
            }
        };

        // 4. Identify redundant pairs
        let mut redundant_pairs = Vec::new();
        for i in 0..num_features {
            for j in (i + 1)..num_features {
                let r = matrix_values[i][j];
                let abs_r = r.abs();
                if abs_r >= threshold {
                    let level = if abs_r >= 0.999 {
                        RedundancyLevel::PerfectlyRedundant
                    } else {
                        RedundancyLevel::HighlyCorrelated
                    };
                    redundant_pairs.push(RedundantFeaturePair {
                        feature_a: features[i],
                        feature_b: features[j],
                        correlation: r,
                        level,
                    });
                }
            }
        }

        // Sort redundant pairs descending by absolute correlation value
        redundant_pairs.sort_by(|a, b| {
            b.correlation
                .abs()
                .partial_cmp(&a.correlation.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(CorrelationReport {
            method,
            matrix: FeatureCorrelationMatrix {
                features: features.clone(),
                values: matrix_values,
            },
            redundant_pairs,
            threshold,
            total_candidates,
        })
    }
}

fn get_feature_value(vector: &FeatureVector, feature: &Feature) -> f64 {
    match feature {
        Feature::AccessFrequency => vector.access_frequency.unwrap_or(0.0),
        Feature::FreshnessDecay => vector.freshness_decay.unwrap_or(0.0),
        Feature::GraphDegree => vector.graph_degree.unwrap_or(0.0),
        Feature::Importance => vector.importance.unwrap_or(0.0),
        Feature::LexicalSimilarity => vector.lexical_similarity.unwrap_or(0.0),
        Feature::ProvenanceConfidence => vector.provenance_confidence.unwrap_or(0.0),
        Feature::Recency => vector.recency.unwrap_or(0.0),
        Feature::SemanticSimilarity => vector.semantic_similarity.unwrap_or(0.0),
    }
}

fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    if n == 0 {
        return 0.0;
    }
    let mean_x = x.iter().sum::<f64>() / (n as f64);
    let mean_y = y.iter().sum::<f64>() / (n as f64);

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for i in 0..n {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    if var_x < 1e-12 || var_y < 1e-12 {
        return 0.0;
    }

    let r = cov / (var_x.sqrt() * var_y.sqrt());
    r.clamp(-1.0, 1.0)
}

fn compute_ranks(values: &[f64]) -> Vec<f64> {
    let n = values.len();
    if n == 0 {
        return Vec::new();
    }
    let mut indexed: Vec<(usize, f64)> = values.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut ranks = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && (indexed[j].1 - indexed[i].1).abs() < 1e-9 {
            j += 1;
        }
        let avg_rank = (i + 1 + j) as f64 / 2.0;
        for k in i..j {
            ranks[indexed[k].0] = avg_rank;
        }
        i = j;
    }
    ranks
}

/// Renderer formatting Pearson/Spearman correlation matrices.
pub struct CorrelationReportWriter;

impl CorrelationReportWriter {
    /// Renders a deterministic Markdown correlation matrix.
    pub fn write_report(report: &CorrelationReport) -> String {
        let mut md = String::new();
        md.push_str("# Feature Correlation & Redundancy Analysis\n\n");
        md.push_str("> [!IMPORTANT]\n");
        md.push_str("> Controlled benchmarks intentionally exaggerate feature influence to verify ranking behavior.\n");
        md.push_str("> Correlation indicates statistical association only. Highly correlated features may still encode distinct causal information.\n\n");

        md.push_str(&format!(
            "Method: **{:?}** | Threshold: **{:.2}** | Total Candidates Checked: **{}**\n\n",
            report.method, report.threshold, report.total_candidates
        ));

        md.push_str("## Correlation Matrix\n\n");

        // Format header
        md.push_str("| Feature |");
        for f in &report.matrix.features {
            md.push_str(&format!(" {} |", f.as_str()));
        }
        md.push_str("\n|");
        for _ in 0..=report.matrix.features.len() {
            md.push_str(" :--- |");
        }
        md.push_str("\n");

        // Format rows
        for (i, row_f) in report.matrix.features.iter().enumerate() {
            md.push_str(&format!("| {} |", row_f.as_str()));
            for j in 0..report.matrix.features.len() {
                md.push_str(&format!(" {:.4} |", report.matrix.values[i][j]));
            }
            md.push_str("\n");
        }

        md.push_str("\n## Redundancy Alerts\n\n");
        if report.redundant_pairs.is_empty() {
            md.push_str("None. No feature pairs exceed the correlation threshold.\n");
        } else {
            md.push_str("| Feature A | Feature B | Correlation | Alert Level |\n");
            md.push_str("| :--- | :--- | ---: | :---: |\n");
            for p in &report.redundant_pairs {
                let alert = match p.level {
                    RedundancyLevel::PerfectlyRedundant => "🚨 Perfectly Redundant",
                    RedundancyLevel::HighlyCorrelated => "⚠️ Highly Correlated",
                };
                md.push_str(&format!(
                    "| {} | {} | {:.4} | {} |\n",
                    p.feature_a.as_str(),
                    p.feature_b.as_str(),
                    p.correlation,
                    alert
                ));
            }
        }

        md
    }
}
