use crate::retrieval::eval_harness::StableReport;

/// Structured differences found between two StableReports.
#[derive(Debug, Clone, Default)]
pub struct StableReportDiff {
    /// List of aggregated metrics that changed.
    pub changed_metrics: Vec<String>,
    /// List of query IDs that had quality metric regressions or changes.
    pub changed_queries: Vec<String>,
}

/// Helper to compare two floats with a tolerance of 1e-9.
fn floats_different(a: f64, b: f64) -> bool {
    (a - b).abs() > 1e-9
}

/// Performs a semantic comparison of current and baseline StableReport structures.
/// Returns Ok(()) if they are identical in metrics and query outcomes, or a detailed Err containing the diff.
pub fn compare_stable_reports(
    current: &StableReport,
    baseline: &StableReport,
) -> Result<(), String> {
    if current.metadata.schema_version != baseline.metadata.schema_version {
        return Err(format!(
            "Schema version mismatch: current={}, baseline={}",
            current.metadata.schema_version, baseline.metadata.schema_version
        ));
    }
    if current.metadata.benchmark_version != baseline.metadata.benchmark_version {
        return Err(format!(
            "Benchmark (corpus) version mismatch: current={}, baseline={}",
            current.metadata.benchmark_version, baseline.metadata.benchmark_version
        ));
    }
    if current.metadata.cache_mode != baseline.metadata.cache_mode {
        return Err(format!(
            "Cache mode mismatch: current={}, baseline={}",
            current.metadata.cache_mode, baseline.metadata.cache_mode
        ));
    }

    let mut diff = StableReportDiff::default();

    // 1. Compare aggregate metrics
    let m_curr = &current.metrics;
    let m_base = &baseline.metrics;

    if floats_different(m_curr.mean_recall_at_1, m_base.mean_recall_at_1) {
        diff.changed_metrics.push("mean_recall_at_1".to_string());
    }
    if floats_different(m_curr.mean_recall_at_5, m_base.mean_recall_at_5) {
        diff.changed_metrics.push("mean_recall_at_5".to_string());
    }
    if floats_different(m_curr.mean_recall_at_10, m_base.mean_recall_at_10) {
        diff.changed_metrics.push("mean_recall_at_10".to_string());
    }
    if floats_different(m_curr.mean_precision_at_5, m_base.mean_precision_at_5) {
        diff.changed_metrics.push("mean_precision_at_5".to_string());
    }
    if floats_different(m_curr.mean_precision_at_10, m_base.mean_precision_at_10) {
        diff.changed_metrics
            .push("mean_precision_at_10".to_string());
    }
    if floats_different(m_curr.mean_mrr, m_base.mean_mrr) {
        diff.changed_metrics.push("mean_mrr".to_string());
    }
    if floats_different(m_curr.mean_ndcg_at_5, m_base.mean_ndcg_at_5) {
        diff.changed_metrics.push("mean_ndcg_at_5".to_string());
    }
    if floats_different(m_curr.mean_ndcg_at_10, m_base.mean_ndcg_at_10) {
        diff.changed_metrics.push("mean_ndcg_at_10".to_string());
    }
    if m_curr.total_queries != m_base.total_queries {
        diff.changed_metrics.push("total_queries".to_string());
    }
    if m_curr.successful_queries != m_base.successful_queries {
        diff.changed_metrics.push("successful_queries".to_string());
    }
    if m_curr.failed_queries != m_base.failed_queries {
        diff.changed_metrics.push("failed_queries".to_string());
    }

    // 2. Compare query-by-query results
    let mut current_map = std::collections::HashMap::new();
    for q in &current.query_results {
        current_map.insert(&q.query_id, q);
    }

    let mut baseline_map = std::collections::HashMap::new();
    for q in &baseline.query_results {
        baseline_map.insert(&q.query_id, q);
    }

    // Gather all query IDs from both maps to find additions, deletions, or modifications
    let mut all_query_ids: Vec<&String> = current_map.keys().copied().collect();
    for k in baseline_map.keys() {
        if !current_map.contains_key(k) {
            all_query_ids.push(k);
        }
    }
    all_query_ids.sort();
    all_query_ids.dedup();

    for q_id in all_query_ids {
        match (current_map.get(q_id), baseline_map.get(q_id)) {
            (None, Some(_)) | (Some(_), None) => {
                diff.changed_queries.push(q_id.to_string());
            }
            (Some(c), Some(b)) => {
                let mut changed = false;
                if c.status != b.status {
                    changed = true;
                }
                if c.error != b.error {
                    changed = true;
                }
                if floats_different(c.recall_at_1, b.recall_at_1) {
                    changed = true;
                }
                if floats_different(c.recall_at_5, b.recall_at_5) {
                    changed = true;
                }
                if floats_different(c.recall_at_10, b.recall_at_10) {
                    changed = true;
                }
                if floats_different(c.precision_at_5, b.precision_at_5) {
                    changed = true;
                }
                if floats_different(c.precision_at_10, b.precision_at_10) {
                    changed = true;
                }
                if floats_different(c.mrr, b.mrr) {
                    changed = true;
                }
                if floats_different(c.ndcg_at_5, b.ndcg_at_5) {
                    changed = true;
                }
                if floats_different(c.ndcg_at_10, b.ndcg_at_10) {
                    changed = true;
                }
                if c.retrieved_count != b.retrieved_count {
                    changed = true;
                }

                if changed {
                    diff.changed_queries.push(q_id.to_string());
                }
            }
            _ => {}
        }
    }

    if diff.changed_metrics.is_empty() && diff.changed_queries.is_empty() {
        return Ok(());
    }

    // Sort to ensure deterministic output format
    let mut metrics = diff.changed_metrics;
    metrics.sort();
    let mut queries = diff.changed_queries;
    queries.sort();

    // Format human-readable regression error message
    let mut msg = String::new();
    msg.push_str("Stable benchmark regression detected.\n");

    if !metrics.is_empty() {
        msg.push_str("\nMetrics:\n");
        for m in metrics {
            msg.push_str(&format!("- {}\n", m));
        }
    }

    if !queries.is_empty() {
        msg.push_str("\nQueries:\n");
        for q in queries {
            msg.push_str(&format!("- {}\n", q));
        }
    }

    msg.push_str("\nRun:\ncargo xtask regenerate-retrieval-baselines\n");

    Err(msg)
}
