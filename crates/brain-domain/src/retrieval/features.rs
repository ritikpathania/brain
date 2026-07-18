use crate::consolidation::MetricConstructionError;
use crate::identifiers::NodeId;
use crate::retrieval::models::{NormalizedSignal, RankingSignals};

/// Holds raw numerical features extracted for a candidate node.
#[derive(Debug, Clone, PartialEq)]
pub struct RawFeatureVector {
    /// Raw semantic match score.
    pub semantic: f64,
    /// Raw graph centrality degree.
    pub graph: f64,
    /// Raw time decay preference weight.
    pub recency: f64,
    /// Raw total temporal edge observation count.
    pub temporal: f64,
}

/// Explicit context/strategy details for normalizing features.
#[derive(Debug, Clone, PartialEq)]
pub enum NormalizationContext {
    /// Normalize dynamically using min-max values of the active batch.
    BatchMinMax,
    /// Normalize using fixed ranges.
    FixedRanges {
        /// Semantic score bounds.
        semantic_range: (f64, f64),
        /// Graph score bounds.
        graph_range: (f64, f64),
        /// Recency decay bounds.
        recency_range: (f64, f64),
        /// Temporal density bounds.
        temporal_range: (f64, f64),
    },
}

/// Trait defining normalization strategies for scaling raw features.
pub trait FeatureNormalizer: Send + Sync {
    /// Normalizes raw features into ranking signal value objects.
    fn normalize(
        &self,
        raw: &[RawFeatureVector],
        context: &NormalizationContext,
    ) -> Result<Vec<RankingSignals>, MetricConstructionError>;
}

/// Min-max scaling normalizer mapping ranges to [0.0, 1.0].
pub struct MinMaxNormalizer;

impl FeatureNormalizer for MinMaxNormalizer {
    fn normalize(
        &self,
        raw: &[RawFeatureVector],
        context: &NormalizationContext,
    ) -> Result<Vec<RankingSignals>, MetricConstructionError> {
        if raw.is_empty() {
            return Ok(Vec::new());
        }

        let (min_sem, max_sem, min_graph, max_graph, min_rec, max_rec, min_temp, max_temp) =
            match context {
                NormalizationContext::BatchMinMax => {
                    let min_s = raw.iter().map(|v| v.semantic).fold(f64::INFINITY, f64::min);
                    let max_s = raw
                        .iter()
                        .map(|v| v.semantic)
                        .fold(f64::NEG_INFINITY, f64::max);
                    let min_g = raw.iter().map(|v| v.graph).fold(f64::INFINITY, f64::min);
                    let max_g = raw
                        .iter()
                        .map(|v| v.graph)
                        .fold(f64::NEG_INFINITY, f64::max);
                    let min_r = raw.iter().map(|v| v.recency).fold(f64::INFINITY, f64::min);
                    let max_r = raw
                        .iter()
                        .map(|v| v.recency)
                        .fold(f64::NEG_INFINITY, f64::max);
                    let min_t = raw.iter().map(|v| v.temporal).fold(f64::INFINITY, f64::min);
                    let max_t = raw
                        .iter()
                        .map(|v| v.temporal)
                        .fold(f64::NEG_INFINITY, f64::max);
                    (min_s, max_s, min_g, max_g, min_r, max_r, min_t, max_t)
                }
                NormalizationContext::FixedRanges {
                    semantic_range,
                    graph_range,
                    recency_range,
                    temporal_range,
                } => (
                    semantic_range.0,
                    semantic_range.1,
                    graph_range.0,
                    graph_range.1,
                    recency_range.0,
                    recency_range.1,
                    temporal_range.0,
                    temporal_range.1,
                ),
            };

        let norm = |val: f64, min: f64, max: f64| -> f64 {
            if max == min || val.is_nan() {
                1.0
            } else {
                ((val - min) / (max - min)).clamp(0.0, 1.0)
            }
        };

        let mut result = Vec::with_capacity(raw.len());
        for v in raw {
            let sem = NormalizedSignal::new(norm(v.semantic, min_sem, max_sem))?;
            let graph = NormalizedSignal::new(norm(v.graph, min_graph, max_graph))?;
            let rec = NormalizedSignal::new(norm(v.recency, min_rec, max_rec))?;
            let temp = NormalizedSignal::new(norm(v.temporal, min_temp, max_temp))?;

            result.push(RankingSignals::new(sem, graph, rec, temp));
        }
        Ok(result)
    }
}

/// Provenance audit trail documenting raw-to-normalized feature transformations.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureExtractionReport {
    /// Target candidate node.
    pub node_id: NodeId,
    /// Originally computed raw feature vector.
    pub raw_features: RawFeatureVector,
    /// Normalization context configurations.
    pub normalization_context: NormalizationContext,
    /// Generated normalized signal parameters.
    pub normalized_signals: RankingSignals,
}

/// Utility to build feature reports compile-time separated from extractors and normalizers.
pub struct FeaturePipelineReporter;

impl FeaturePipelineReporter {
    /// Build feature extraction reports from inputs and outputs.
    pub fn build_reports(
        nodes: &[crate::Node],
        raw: &[RawFeatureVector],
        normalized: &[RankingSignals],
        context: &NormalizationContext,
    ) -> Vec<FeatureExtractionReport> {
        let mut reports = Vec::with_capacity(raw.len());
        for i in 0..raw.len() {
            reports.push(FeatureExtractionReport {
                node_id: nodes[i].id,
                raw_features: raw[i].clone(),
                normalization_context: context.clone(),
                normalized_signals: normalized[i].clone(),
            });
        }
        reports
    }
}
