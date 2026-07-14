/// Evaluation metrics sub-engine.
pub mod metrics;
/// Evaluation benchmark runner.
pub mod runner;
/// FTS retriever implementation.
pub mod fts_retriever;
/// Regression analyzer.
pub mod regression;
/// Semantic retriever implementation.
pub mod semantic_retriever;
/// Hybrid retriever implementation.
pub mod hybrid_retriever;
/// Linear ranking and feature extraction.
pub mod ranking;
/// Feature context metadata provider.
pub mod provider;
/// Calibration and weights optimization engine.
pub mod calibration;
/// Sensitivity and feature diagnostics engine.
pub mod sensitivity;

pub use calibration::{
    CalibrationObjective, CalibrationOptions, CalibrationResult, EvaluationSession,
    CalibrationEngine, MarkdownReportWriter, QueryEvaluationCache,
};
pub use sensitivity::{
    run_sensitivity_analysis, FeatureImpact, SensitivityReport, SensitivityReportWriter,
};

pub use fts_retriever::FtsRetriever;
pub use runner::{
    run_benchmark, BenchmarkReport, StableReport, MeasuredReport, AggregateMetrics,
    AggregateLatency, QueryEvalResult, QueryDiagnostic, CandidateDiagnostic,
};
pub use regression::{compare_stable_reports, StableReportDiff};
pub use semantic_retriever::SemanticRetriever;
pub use hybrid_retriever::HybridRetriever;
pub use ranking::{RankingWeights, LinearRanker, RankingRetriever};
pub use provider::FeatureProvider;

use brain_core::errors::BrainError;
use brain_domain::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Classification of the search channel originating the candidate node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetrievalChannel {
    /// Full-text lexical search channel.
    Fts,
    /// Vector database semantic search channel.
    Semantic,
    /// Structural metadata filtering channel.
    Metadata,
}

/// Representation of a single candidate returned by a retriever.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalResult {
    /// The unique identifier of the retrieved memory node.
    pub node_id: NodeId,
    /// The channel-local scores for each retrieval origin.
    #[serde(default)]
    pub channel_scores: HashMap<RetrievalChannel, f64>,
    /// The computed ranking score, if this result was run through a ranker.
    #[serde(default)]
    pub ranking_score: Option<f64>,
}

impl RetrievalResult {
    /// Returns the score for a specific retrieval channel, if present.
    pub fn score(&self, channel: RetrievalChannel) -> Option<f64> {
        self.channel_scores.get(&channel).copied()
    }

    /// Returns a sorted list of all retrieval channels.
    pub fn channels(&self) -> Vec<RetrievalChannel> {
        let mut chs: Vec<RetrievalChannel> = self.channel_scores.keys().copied().collect();
        chs.sort();
        chs
    }

    /// Checks if the candidate was retrieved by a specific channel.
    pub fn has_channel(&self, channel: RetrievalChannel) -> bool {
        self.channel_scores.contains_key(&channel)
    }
}

/// Abstract contract for candidate retrievers.
pub trait Retriever {
    /// Queries the search index and returns the set of candidate matches.
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError>;

    /// Optional hook to expose the normalized representation of a query.
    fn normalize_query(&self, _query: &str) -> Option<String> {
        None
    }

    /// Optional hook to expose the executed backend query string.
    fn executed_query(&self, _query: &str) -> Option<String> {
        None
    }
}

/// Helper function to deterministically sort retrieval results.
/// - If any candidate contains a `ranking_score`, sorts descending by `ranking_score` (falling back to node_id ascending on ties).
/// - If all results share exactly one common channel, sorts descending by that channel's score.
/// - Otherwise (hybrid/union), sorts by node_id ascending.
pub fn sort_results_deterministically(results: &mut [RetrievalResult]) {
    if results.is_empty() {
        return;
    }

    let has_ranked = results.iter().any(|r| r.ranking_score.is_some());
    if has_ranked {
        results.sort_by(|a, b| {
            let score_a = a.ranking_score.unwrap_or(std::f64::NEG_INFINITY);
            let score_b = b.ranking_score.unwrap_or(std::f64::NEG_INFINITY);

            match score_b.partial_cmp(&score_a) {
                Some(std::cmp::Ordering::Equal) | None => a.node_id.cmp(&b.node_id),
                Some(ord) => ord,
            }
        });
        return;
    }

    let mut common_channel = None;
    let mut first = true;

    for res in results.iter() {
        if first {
            if res.channel_scores.len() == 1 {
                common_channel = res.channel_scores.keys().next().copied();
            }
            first = false;
        } else {
            if res.channel_scores.len() != 1 || res.channel_scores.keys().next().copied() != common_channel {
                common_channel = None;
                break;
            }
        }
    }

    if let Some(channel) = common_channel {
        results.sort_by(|a, b| {
            let score_a = a.score(channel).unwrap_or(std::f64::NEG_INFINITY);
            let score_b = b.score(channel).unwrap_or(std::f64::NEG_INFINITY);

            match score_b.partial_cmp(&score_a) {
                Some(std::cmp::Ordering::Equal) | None => a.node_id.cmp(&b.node_id),
                Some(ord) => ord,
            }
        });
    } else {
        results.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    }
}

/// JSON payload struct for queries.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCorpus {
    /// Schema version
    pub version: u64,
    /// List of benchmark queries
    pub queries: Vec<QueryItem>,
}

/// Single query item in queries.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryItem {
    /// Unique identifier for this query (e.g. q_001)
    pub query_id: String,
    /// Raw search query text
    pub text: String,
    /// List of tags/categories (e.g. ["sdk", "typo"])
    pub tags: Vec<String>,
}

/// JSON payload struct for ground_truth.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruthCorpus {
    /// Schema version
    pub version: u64,
    /// List of source memory nodes to populate the test database
    pub nodes: Vec<CorpusNode>,
    /// Map of query_id to its expected matching items
    pub ground_truth: HashMap<String, GroundTruthItem>,
}

/// Memory node definition in the corpus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusNode {
    /// The raw NodeId string representation
    pub node_id: String,
    /// The text content of the memory node
    pub content: String,
    /// The node type (e.g., Concept, Observation)
    #[serde(rename = "type")]
    pub node_type: String,
}

/// Ground truth constraints for a single query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruthItem {
    /// Memory node IDs expected to be found
    pub expected_node_ids: Vec<String>,
    /// Acceptable alternative node IDs (counted in Precision, not Recall)
    pub acceptable_alternatives: Vec<String>,
    /// Target minimum rank index constraint (optional)
    pub minimum_rank: HashMap<String, usize>,
}

/// Validates the structure and sanity constraints of the evaluation corpus.
pub fn validate_corpus(queries: &QueryCorpus, truth: &GroundTruthCorpus) -> Result<(), String> {
    if queries.version != truth.version {
        return Err(format!(
            "Corpus version mismatch: queries.json is v{}, ground_truth.json is v{}",
            queries.version, truth.version
        ));
    }

    // 1. Verify unique query IDs
    let mut query_ids = HashSet::new();
    for q in &queries.queries {
        if !query_ids.insert(&q.query_id) {
            return Err(format!("Duplicate query_id found: {}", q.query_id));
        }
    }

    // 2. Verify unique node IDs
    let mut node_ids = HashSet::new();
    for node in &truth.nodes {
        if !node_ids.insert(&node.node_id) {
            return Err(format!("Duplicate node_id found: {}", node.node_id));
        }
    }

    // 3. Verify ground truth items map to unique query IDs
    for q_id in &query_ids {
        let truth_item = match truth.ground_truth.get(*q_id) {
            Some(item) => item,
            None => return Err(format!("Missing ground truth mapping for query_id: {}", q_id)),
        };

        if truth_item.expected_node_ids.is_empty() {
            return Err(format!("Query {} must have at least one expected node_id", q_id));
        }

        // 4. Verify all referenced nodes exist in the nodes array
        for expected in &truth_item.expected_node_ids {
            if !node_ids.contains(expected) {
                return Err(format!(
                    "Query {} references expected node_id {} which does not exist in nodes list",
                    q_id, expected
                ));
            }
        }
        for alt in &truth_item.acceptable_alternatives {
            if !node_ids.contains(alt) {
                return Err(format!(
                    "Query {} references alternative node_id {} which does not exist in nodes list",
                    q_id, alt
                ));
            }
        }
    }

    Ok(())
}

/// Representation of extracted features for a candidate retrieved memory node.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureVector {
    /// Score from lexical Full-Text Search, if discovered via FTS.
    pub lexical_similarity: Option<f64>,
    /// Score from vector semantic similarity, if discovered via Semantic search.
    pub semantic_similarity: Option<f64>,
    /// Temporal recency score (decayed updated_at delta).
    pub recency: Option<f64>,
    /// Combined static importance and pinning flag.
    pub importance: Option<f64>,
    /// Confidence of the source ingestion provenance.
    pub provenance_confidence: Option<f64>,
    /// Log-scaled graph degree.
    pub graph_degree: Option<f64>,
    /// Log-scaled access frequency.
    pub access_frequency: Option<f64>,
    /// Freshness decay (decayed last_observed_at delta).
    pub freshness_decay: Option<f64>,
}

/// Half-life configuration parameters for exponential time decays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankingDecay {
    /// Half-life for updated_at recency decay in days.
    pub recency_half_life_days: f64,
    /// Half-life for freshness decay (based on last edge observation) in days.
    pub freshness_half_life_days: f64,
}

impl Default for RankingDecay {
    fn default() -> Self {
        Self {
            recency_half_life_days: 7.0,   // 1 week half-life
            freshness_half_life_days: 1.0, // 1 day half-life
        }
    }
}

/// Immutable database snapshot context used to construct a feature vector.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureContext {
    /// Timestamp when the node was updated.
    pub updated_at: Option<u64>,
    /// Importance score assigned to the node.
    pub importance: Option<f64>,
    /// Pinned indicator.
    pub pinned: bool,
    /// Provenance confidence score.
    pub provenance_confidence: Option<f64>,
    /// Number of edges connected to this node in the graph.
    pub graph_degree: Option<u32>,
    /// Ingestion/Interaction selection count.
    pub access_count: Option<u64>,
    /// Most recent observed timestamp across all connecting edges.
    pub last_observed_at: Option<u64>,
}

/// Pure translation layer extracting raw channel-local features from candidate RetrievalResult.
pub struct FeatureExtractor {
    /// Reference time point used to calculate age deltas.
    pub reference_time: u64,
    /// Exponential decay parameters.
    pub decay: RankingDecay,
}

impl FeatureExtractor {
    /// Instantiates a new FeatureExtractor with reference time and decay parameters.
    pub fn new(reference_time: u64, decay: RankingDecay) -> Self {
        Self { reference_time, decay }
    }

    /// Purely extracts a FeatureVector from retrieval evidence and database context.
    /// Feature extraction is pure: it derives features only from retrieval evidence and immutable metadata.
    /// It must not perform additional retrieval, graph traversal, or ranking.
    pub fn extract(&self, result: &RetrievalResult, context: &FeatureContext) -> FeatureVector {
        let lexical_similarity = result.score(RetrievalChannel::Fts);
        let semantic_similarity = result.score(RetrievalChannel::Semantic);

        // 1. Recency Decay
        let recency = context.updated_at.map(|updated_at| {
            let dt = (self.reference_time.saturating_sub(updated_at)) as f64;
            let half_life_sec = self.decay.recency_half_life_days * 86400.0;
            if half_life_sec <= 0.0 {
                1.0
            } else {
                let tau = half_life_sec / 2.0f64.ln();
                (-dt / tau).exp()
            }
        });

        // 2. Importance
        let importance = if context.pinned {
            Some(1.0)
        } else {
            context.importance.or(Some(0.0))
        };

        // 3. Provenance Confidence
        let provenance_confidence = context.provenance_confidence.or(Some(1.0));

        // 4. Log-scaled Graph Degree
        let graph_degree = context.graph_degree.map(|degree| (degree as f64 + 1.0).ln());

        // 5. Log-scaled Access Frequency
        let access_frequency = context.access_count.map(|count| (count as f64 + 1.0).ln());

        // 6. Freshness Decay
        let freshness_decay = context.last_observed_at.map(|last_observed| {
            let dt = (self.reference_time.saturating_sub(last_observed)) as f64;
            let half_life_sec = self.decay.freshness_half_life_days * 86400.0;
            if half_life_sec <= 0.0 {
                1.0
            } else {
                let tau = half_life_sec / 2.0f64.ln();
                (-dt / tau).exp()
            }
        });

        FeatureVector {
            lexical_similarity,
            semantic_similarity,
            recency,
            importance,
            provenance_confidence,
            graph_degree,
            access_frequency,
            freshness_decay,
        }
    }
}
