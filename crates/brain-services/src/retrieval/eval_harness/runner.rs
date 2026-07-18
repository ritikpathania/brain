use crate::retrieval::eval_harness::metrics::{
    compute_mrr, compute_ndcg_at_k, compute_precision_at_k, compute_recall_at_k,
};
use crate::retrieval::eval_harness::{
    sort_results_deterministically, validate_corpus, GroundTruthCorpus, QueryCorpus,
    RetrievalChannel, Retriever,
};
use brain_domain::NodeId;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Machine-readable benchmark evaluation report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkReport {
    /// Reproducible quality metrics (machine-independent).
    pub stable: StableReport,
    /// Measured performance metrics (machine-dependent).
    pub measured: MeasuredReport,
}

/// Metadata, overall metrics, and query-by-query quality metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StableReport {
    /// Report metadata.
    pub metadata: ReportMetadata,
    /// Aggregated overall metrics across all successful query runs.
    pub metrics: AggregateMetrics,
    /// Individual query evaluation quality results.
    pub query_results: Vec<QueryEvalResult>,
}

/// Measured latencies and candidates details.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeasuredReport {
    /// Aggregated latency statistics.
    pub latency: AggregateLatency,
    /// Individual query execution diagnostics.
    pub diagnostics: Vec<QueryDiagnostic>,
}

/// Report metadata context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportMetadata {
    /// Version of the report format schema.
    pub schema_version: u64,
    /// Version of the benchmark corpus evaluated.
    pub benchmark_version: u64,
    /// Identifier of the harness runner.
    pub generated_by: String,
    /// The cache mode evaluated ("cold" or "warm").
    pub cache_mode: String,
}

/// Aggregated quality metrics.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AggregateMetrics {
    /// Mean Recall@1.
    pub mean_recall_at_1: f64,
    /// Mean Recall@5.
    pub mean_recall_at_5: f64,
    /// Mean Recall@10.
    pub mean_recall_at_10: f64,
    /// Mean Precision@5.
    pub mean_precision_at_5: f64,
    /// Mean Precision@10.
    pub mean_precision_at_10: f64,
    /// Mean Reciprocal Rank (MRR).
    pub mean_mrr: f64,
    /// Mean Normalized Discounted Cumulative Gain (nDCG@5).
    #[serde(default)]
    pub mean_ndcg_at_5: f64,
    /// Mean Normalized Discounted Cumulative Gain (nDCG@10).
    #[serde(default)]
    pub mean_ndcg_at_10: f64,
    /// Total number of queries evaluated.
    pub total_queries: usize,
    /// Number of queries that succeeded.
    pub successful_queries: usize,
    /// Number of queries that failed.
    pub failed_queries: usize,
}

/// Aggregated latency metrics.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AggregateLatency {
    /// Median latency in milliseconds.
    pub p50_latency_ms: f64,
    /// 95th percentile latency in milliseconds.
    pub p95_latency_ms: f64,
    /// Maximum latency in milliseconds.
    pub max_latency_ms: f64,
}

/// Detailed diagnostic record for a single retrieved candidate node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateDiagnostic {
    /// Unique identifier of the node.
    pub node_id: String,
    /// The raw lexical score computed by FTS.
    #[serde(default)]
    pub lexical_score: Option<f64>,
    /// The raw semantic similarity score computed by Cosine Similarity.
    #[serde(default)]
    pub semantic_score: Option<f64>,
    /// The computed ranking score, if applicable.
    #[serde(default)]
    pub ranked_score: Option<f64>,
    /// List of channels that retrieved this candidate.
    pub retrieval_channels: Vec<RetrievalChannel>,
}

/// Quality metrics for a single query (stable / machine-independent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryEvalResult {
    /// Unique identifier of the query.
    pub query_id: String,
    /// Query text.
    pub text: String,
    /// Run status: "success" or "retrieval_error".
    pub status: String,
    /// Error message if retrieval failed.
    pub error: Option<String>,
    /// Recall@1.
    pub recall_at_1: f64,
    /// Recall@5.
    pub recall_at_5: f64,
    /// Recall@10.
    pub recall_at_10: f64,
    /// Precision@5.
    pub precision_at_5: f64,
    /// Precision@10.
    pub precision_at_10: f64,
    /// Reciprocal Rank.
    pub mrr: f64,
    /// Normalized Discounted Cumulative Gain (nDCG@5).
    #[serde(default)]
    pub ndcg_at_5: f64,
    /// Normalized Discounted Cumulative Gain (nDCG@10).
    #[serde(default)]
    pub ndcg_at_10: f64,
    /// Total count of retrieved candidates.
    pub retrieved_count: usize,
}

/// Detailed runtime diagnostic record for a single query (measured / machine-dependent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryDiagnostic {
    /// Unique identifier of the query.
    pub query_id: String,
    /// Query latency in milliseconds.
    pub latency_ms: f64,
    /// Optional normalized representation of the query.
    #[serde(default)]
    pub normalized_query: Option<String>,
    /// Optional executed query string passed to search engine.
    #[serde(default)]
    pub executed_query: Option<String>,
    /// List of retrieved candidate diagnostics.
    #[serde(default)]
    pub candidates: Vec<CandidateDiagnostic>,
}

/// Executes the evaluation corpus benchmark against an injected Retriever.
pub fn run_benchmark(
    queries_json: &str,
    ground_truth_json: &str,
    retriever: &dyn Retriever,
    cache_mode: &str,
) -> Result<BenchmarkReport, String> {
    // 1. Parse evaluation corpus files (Infrastructure step - failures here are fatal)
    let queries: QueryCorpus = serde_json::from_str(queries_json)
        .map_err(|e| format!("Fatal: Failed to parse queries.json: {}", e))?;
    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json)
        .map_err(|e| format!("Fatal: Failed to parse ground_truth.json: {}", e))?;

    // 2. Validate corpus integrity
    validate_corpus(&queries, &ground_truth)?;

    let mut query_results = Vec::new();
    let mut diagnostics = Vec::new();
    let mut success_count = 0;
    let mut fail_count = 0;

    let mut sum_recall_at_1 = 0.0;
    let mut sum_recall_at_5 = 0.0;
    let mut sum_recall_at_10 = 0.0;
    let mut sum_precision_at_5 = 0.0;
    let mut sum_precision_at_10 = 0.0;
    let mut sum_mrr = 0.0;
    let mut sum_ndcg_at_5 = 0.0;
    let mut sum_ndcg_at_10 = 0.0;

    let mut latencies = Vec::new();

    // Map helper to lookup expected nodes by string ID
    let string_to_node_id = |s: &str| -> Result<NodeId, String> {
        let uuid = uuid::Uuid::parse_str(s)
            .map_err(|e| format!("Invalid node UUID in ground truth: {}", e))?;
        Ok(NodeId(uuid))
    };

    for query in &queries.queries {
        let truth = ground_truth.ground_truth.get(&query.query_id).unwrap();

        let expected_node_ids = truth
            .expected_node_ids
            .iter()
            .map(|s| string_to_node_id(s))
            .collect::<Result<Vec<NodeId>, String>>()?;
        let acceptable_alternatives = truth
            .acceptable_alternatives
            .iter()
            .map(|s| string_to_node_id(s))
            .collect::<Result<Vec<NodeId>, String>>()?;

        // Extract query normalization diagnostics
        let normalized_query = retriever.normalize_query(&query.text);
        let executed_query = retriever.executed_query(&query.text);

        // 3. Query the retriever (Retriever errors do NOT abort the benchmark)
        let start_time = Instant::now();
        let ret_res = retriever.retrieve(&query.text);
        let elapsed = start_time.elapsed().as_secs_f64() * 1000.0;

        match ret_res {
            Ok(mut results) => {
                // Apply deterministic tie-breaking logic
                sort_results_deterministically(&mut results);
                let retrieved_ids: Vec<NodeId> = results.iter().map(|r| r.node_id).collect();

                let recall_at_1 = compute_recall_at_k(&retrieved_ids, &expected_node_ids, 1);
                let recall_at_5 = compute_recall_at_k(&retrieved_ids, &expected_node_ids, 5);
                let recall_at_10 = compute_recall_at_k(&retrieved_ids, &expected_node_ids, 10);
                let precision_at_5 = compute_precision_at_k(
                    &retrieved_ids,
                    &expected_node_ids,
                    &acceptable_alternatives,
                    5,
                );
                let precision_at_10 = compute_precision_at_k(
                    &retrieved_ids,
                    &expected_node_ids,
                    &acceptable_alternatives,
                    10,
                );
                let mrr = compute_mrr(&retrieved_ids, &expected_node_ids, &acceptable_alternatives);
                let ndcg_at_5 = compute_ndcg_at_k(
                    &retrieved_ids,
                    &expected_node_ids,
                    &acceptable_alternatives,
                    5,
                );
                let ndcg_at_10 = compute_ndcg_at_k(
                    &retrieved_ids,
                    &expected_node_ids,
                    &acceptable_alternatives,
                    10,
                );

                sum_recall_at_1 += recall_at_1;
                sum_recall_at_5 += recall_at_5;
                sum_recall_at_10 += recall_at_10;
                sum_precision_at_5 += precision_at_5;
                sum_precision_at_10 += precision_at_10;
                sum_mrr += mrr;
                sum_ndcg_at_5 += ndcg_at_5;
                sum_ndcg_at_10 += ndcg_at_10;
                success_count += 1;
                latencies.push(elapsed);

                // Build candidate diagnostics
                let mut candidates = Vec::with_capacity(results.len());
                for res in &results {
                    candidates.push(CandidateDiagnostic {
                        node_id: res.node_id.to_string(),
                        lexical_score: res.score(RetrievalChannel::Fts),
                        semantic_score: res.score(RetrievalChannel::Semantic),
                        ranked_score: res.ranking_score,
                        retrieval_channels: res.channels(),
                    });
                }

                query_results.push(QueryEvalResult {
                    query_id: query.query_id.clone(),
                    text: query.text.clone(),
                    status: "success".to_string(),
                    error: None,
                    recall_at_1,
                    recall_at_5,
                    recall_at_10,
                    precision_at_5,
                    precision_at_10,
                    mrr,
                    ndcg_at_5,
                    ndcg_at_10,
                    retrieved_count: retrieved_ids.len(),
                });

                diagnostics.push(QueryDiagnostic {
                    query_id: query.query_id.clone(),
                    latency_ms: elapsed,
                    normalized_query,
                    executed_query,
                    candidates,
                });
            }
            Err(e) => {
                fail_count += 1;
                query_results.push(QueryEvalResult {
                    query_id: query.query_id.clone(),
                    text: query.text.clone(),
                    status: "retrieval_error".to_string(),
                    error: Some(format!("{:?}", e)),
                    recall_at_1: 0.0,
                    recall_at_5: 0.0,
                    recall_at_10: 0.0,
                    precision_at_5: 0.0,
                    precision_at_10: 0.0,
                    mrr: 0.0,
                    ndcg_at_5: 0.0,
                    ndcg_at_10: 0.0,
                    retrieved_count: 0,
                });

                diagnostics.push(QueryDiagnostic {
                    query_id: query.query_id.clone(),
                    latency_ms: elapsed,
                    normalized_query,
                    executed_query,
                    candidates: Vec::new(),
                });
            }
        }
    }

    let mut p50 = 0.0;
    let mut p95 = 0.0;
    let mut max = 0.0;

    if !latencies.is_empty() {
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        max = latencies[latencies.len() - 1];

        let p50_idx = (0.50 * (latencies.len() - 1) as f64).round() as usize;
        p50 = latencies[p50_idx];

        let p95_idx = (0.95 * (latencies.len() - 1) as f64).round() as usize;
        p95 = latencies[p95_idx];
    }

    let total = queries.queries.len();
    let overall_metrics = if success_count > 0 {
        let count_f = success_count as f64;
        AggregateMetrics {
            mean_recall_at_1: sum_recall_at_1 / count_f,
            mean_recall_at_5: sum_recall_at_5 / count_f,
            mean_recall_at_10: sum_recall_at_10 / count_f,
            mean_precision_at_5: sum_precision_at_5 / count_f,
            mean_precision_at_10: sum_precision_at_10 / count_f,
            mean_mrr: sum_mrr / count_f,
            mean_ndcg_at_5: sum_ndcg_at_5 / count_f,
            mean_ndcg_at_10: sum_ndcg_at_10 / count_f,
            total_queries: total,
            successful_queries: success_count,
            failed_queries: fail_count,
        }
    } else {
        AggregateMetrics {
            total_queries: total,
            failed_queries: fail_count,
            ..Default::default()
        }
    };

    let latency = AggregateLatency {
        p50_latency_ms: p50,
        p95_latency_ms: p95,
        max_latency_ms: max,
    };

    Ok(BenchmarkReport {
        stable: StableReport {
            metadata: ReportMetadata {
                schema_version: 2,
                benchmark_version: queries.version,
                generated_by: "brain-evaluation-harness".to_string(),
                cache_mode: cache_mode.to_string(),
            },
            metrics: overall_metrics,
            query_results,
        },
        measured: MeasuredReport {
            latency,
            diagnostics,
        },
    })
}
