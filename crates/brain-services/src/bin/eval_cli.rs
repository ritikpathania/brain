use brain_core::repositories::RepositorySet;
use brain_services::retrieval::eval_harness::{
    run_benchmark, AggregateLatency, AggregateMetrics, GraphAwareRetriever,
    HashingEmbeddingProvider, HybridRetriever, ProductionCorpusBuilder, SemanticRetriever,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize)]
struct EvalReport {
    schema_version: u64,
    brain_version: String,
    git_commit: String,
    corpus_version: u64,
    fusion_strategy: String,
    generated_at: String,
    metrics: AggregateMetrics,
    latency: AggregateLatency,
    corpus_stats: CorpusStats,
    runtime_stats: RuntimeStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    graph_evaluation: Option<std::collections::HashMap<String, GraphEvalMetrics>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GraphEvalMetrics {
    metrics: AggregateMetrics,
    latency: AggregateLatency,
}

#[derive(Debug, Serialize, Deserialize)]
struct CorpusStats {
    total_nodes: usize,
    total_queries: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct RuntimeStats {
    evaluation_runtime_ms: u64,
    db_size_bytes: u64,
}

fn get_git_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .map(|s| s.trim().to_string())
                    .ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn check_regressions(current: &EvalReport, baseline: &EvalReport) -> Result<(), Vec<String>> {
    let mut regressions = Vec::new();
    let cur_m = &current.metrics;
    let bas_m = &baseline.metrics;

    let eps = 1e-9;

    if cur_m.mean_recall_at_1 < bas_m.mean_recall_at_1 - eps {
        regressions.push(format!(
            "mean_recall_at_1: current = {}, baseline = {}",
            cur_m.mean_recall_at_1, bas_m.mean_recall_at_1
        ));
    }
    if cur_m.mean_recall_at_5 < bas_m.mean_recall_at_5 - eps {
        regressions.push(format!(
            "mean_recall_at_5: current = {}, baseline = {}",
            cur_m.mean_recall_at_5, bas_m.mean_recall_at_5
        ));
    }
    if cur_m.mean_recall_at_10 < bas_m.mean_recall_at_10 - eps {
        regressions.push(format!(
            "mean_recall_at_10: current = {}, baseline = {}",
            cur_m.mean_recall_at_10, bas_m.mean_recall_at_10
        ));
    }
    if cur_m.mean_precision_at_5 < bas_m.mean_precision_at_5 - eps {
        regressions.push(format!(
            "mean_precision_at_5: current = {}, baseline = {}",
            cur_m.mean_precision_at_5, bas_m.mean_precision_at_5
        ));
    }
    if cur_m.mean_precision_at_10 < bas_m.mean_precision_at_10 - eps {
        regressions.push(format!(
            "mean_precision_at_10: current = {}, baseline = {}",
            cur_m.mean_precision_at_10, bas_m.mean_precision_at_10
        ));
    }
    if cur_m.mean_mrr < bas_m.mean_mrr - eps {
        regressions.push(format!(
            "mean_mrr: current = {}, baseline = {}",
            cur_m.mean_mrr, bas_m.mean_mrr
        ));
    }
    if cur_m.mean_ndcg_at_5 < bas_m.mean_ndcg_at_5 - eps {
        regressions.push(format!(
            "mean_ndcg_at_5: current = {}, baseline = {}",
            cur_m.mean_ndcg_at_5, bas_m.mean_ndcg_at_5
        ));
    }
    if cur_m.mean_ndcg_at_10 < bas_m.mean_ndcg_at_10 - eps {
        regressions.push(format!(
            "mean_ndcg_at_10: current = {}, baseline = {}",
            cur_m.mean_ndcg_at_10, bas_m.mean_ndcg_at_10
        ));
    }

    if regressions.is_empty() {
        Ok(())
    } else {
        Err(regressions)
    }
}

fn main() {
    let mut baseline_path = None;
    let mut write_path = None;
    let mut graph_depths = Vec::<usize>::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--baseline" => {
                baseline_path = args.next();
            }
            "--write" => {
                write_path = args.next();
            }
            "--graph-depth" => {
                if let Some(depth_str) = args.next() {
                    match depth_str.parse::<usize>() {
                        Ok(d) => graph_depths.push(d),
                        Err(_) => {
                            eprintln!("Invalid graph depth: {}", depth_str);
                            std::process::exit(1);
                        }
                    }
                } else {
                    eprintln!("Missing value for --graph-depth");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Unknown argument: {}", arg);
                std::process::exit(1);
            }
        }
    }

    println!("Building production corpus...");
    let corpus = match ProductionCorpusBuilder::build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build production corpus: {:?}", e);
            std::process::exit(1);
        }
    };

    let queries_json = serde_json::to_string(&corpus.queries).unwrap();
    let ground_truth_json = serde_json::to_string(&corpus.ground_truth).unwrap();

    println!("Running retrieval benchmarks...");
    let start_time = Instant::now();
    let benchmark_report =
        match run_benchmark(&queries_json, &ground_truth_json, &corpus.retriever, "cold") {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Benchmark execution failed: {}", e);
                std::process::exit(1);
            }
        };
    let evaluation_runtime_ms = start_time.elapsed().as_millis() as u64;

    // Retrieve database size using Page Size * Page Count
    let db_size_bytes = corpus
        .storage
        .pool()
        .get()
        .map_err(|e| e.to_string())
        .and_then(|conn| {
            let page_count: i64 = conn
                .query_row("PRAGMA page_count", [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            let page_size: i64 = conn
                .query_row("PRAGMA page_size", [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            Ok((page_count * page_size) as u64)
        })
        .unwrap_or(0);

    let mut graph_eval = std::collections::HashMap::new();
    let mut graph_table_rows = Vec::new();

    let repos = corpus.storage.clone() as Arc<dyn RepositorySet>;
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());

    for &depth in &graph_depths {
        println!("Running evaluation with graph depth {}...", depth);
        let fts_graph = GraphAwareRetriever::new(repos.clone(), registry.clone(), depth);
        let semantic_retriever = SemanticRetriever::new(
            corpus.storage.pool().clone(),
            Arc::new(HashingEmbeddingProvider),
        );
        let hybrid_graph = HybridRetriever::new(fts_graph, semantic_retriever);

        let g_report = match run_benchmark(&queries_json, &ground_truth_json, &hybrid_graph, "cold")
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Benchmark execution failed for depth {}: {}", depth, e);
                std::process::exit(1);
            }
        };

        let entry = GraphEvalMetrics {
            metrics: g_report.stable.metrics.clone(),
            latency: g_report.measured.latency.clone(),
        };
        graph_eval.insert(format!("depth{}", depth), entry);

        // Calculate delta latency vs baseline p50
        let latency_delta_pct = if benchmark_report.measured.latency.p50_latency_ms > 0.0 {
            ((g_report.measured.latency.p50_latency_ms
                - benchmark_report.measured.latency.p50_latency_ms)
                / benchmark_report.measured.latency.p50_latency_ms)
                * 100.0
        } else {
            0.0
        };

        graph_table_rows.push((
            depth,
            g_report.stable.metrics.mean_recall_at_10,
            g_report.stable.metrics.mean_mrr,
            g_report.stable.metrics.mean_ndcg_at_10,
            latency_delta_pct,
        ));
    }

    let graph_evaluation = if graph_eval.is_empty() {
        None
    } else {
        Some(graph_eval)
    };

    let report = EvalReport {
        schema_version: 2,
        brain_version: "0.8.0".to_string(),
        git_commit: get_git_commit(),
        corpus_version: corpus.queries.version,
        fusion_strategy: "RRF".to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        metrics: benchmark_report.stable.metrics,
        latency: benchmark_report.measured.latency,
        corpus_stats: CorpusStats {
            total_nodes: corpus.ground_truth.nodes.len(),
            total_queries: corpus.queries.queries.len(),
        },
        runtime_stats: RuntimeStats {
            evaluation_runtime_ms,
            db_size_bytes,
        },
        graph_evaluation,
    };

    println!("\n=== Evaluation Results ===");
    println!(
        "Evaluation Runtime: {} ms",
        report.runtime_stats.evaluation_runtime_ms
    );
    println!(
        "Database Size: {} bytes",
        report.runtime_stats.db_size_bytes
    );
    println!("Total Queries: {}", report.corpus_stats.total_queries);
    println!("Total Nodes: {}", report.corpus_stats.total_nodes);
    println!("---------------------------");
    println!("Mean Recall @ 1:   {:.6}", report.metrics.mean_recall_at_1);
    println!("Mean Recall @ 5:   {:.6}", report.metrics.mean_recall_at_5);
    println!("Mean Recall @ 10:  {:.6}", report.metrics.mean_recall_at_10);
    println!(
        "Mean Precision @ 5: {:.6}",
        report.metrics.mean_precision_at_5
    );
    println!(
        "Mean Precision @ 10:{:.6}",
        report.metrics.mean_precision_at_10
    );
    println!("Mean MRR:          {:.6}", report.metrics.mean_mrr);
    println!("Mean nDCG @ 5:     {:.6}", report.metrics.mean_ndcg_at_5);
    println!("Mean nDCG @ 10:    {:.6}", report.metrics.mean_ndcg_at_10);
    println!("===========================");

    if !graph_table_rows.is_empty() {
        println!("\n=== Graph Traversal Depth Comparison ===");
        println!("| Depth | Recall@10 |      MRR |  nDCG@10 | Δ Latency |");
        println!("| ----- | --------: | -------: | --------: | --------: |");
        println!(
            "| 0     |  {:.6} | {:.6} |  {:.6} |  baseline |",
            report.metrics.mean_recall_at_10,
            report.metrics.mean_mrr,
            report.metrics.mean_ndcg_at_10
        );
        for row in &graph_table_rows {
            println!(
                "| {}     |  {:.6} | {:.6} |  {:.6} |    {:+0.1}% |",
                row.0, row.1, row.2, row.3, row.4
            );
        }
        println!("=========================================");
    }

    if let Some(ref path_str) = write_path {
        let path = Path::new(path_str);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let serialized = serde_json::to_string_pretty(&report).unwrap();
        match fs::write(path, serialized) {
            Ok(_) => println!("Report written to {}", path_str),
            Err(e) => eprintln!("Failed to write report to {}: {}", path_str, e),
        }
    }

    if let Some(ref path_str) = baseline_path {
        println!("Comparing against baseline: {}", path_str);
        let baseline_data = match fs::read_to_string(path_str) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to read baseline file: {}", e);
                std::process::exit(1);
            }
        };
        let baseline_report: EvalReport = match serde_json::from_str(&baseline_data) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to parse baseline report: {}", e);
                std::process::exit(1);
            }
        };

        println!("\n=== Baseline vs Current ===");
        println!("| Metric | Baseline | Current | Delta |");
        println!("| --- | --- | --- | --- |");
        let format_row = |name: &str, base: f64, cur: f64| {
            let delta = cur - base;
            format!("| {} | {:.6} | {:.6} | {:+.6} |", name, base, cur, delta)
        };
        println!(
            "{}",
            format_row(
                "Recall @ 1",
                baseline_report.metrics.mean_recall_at_1,
                report.metrics.mean_recall_at_1
            )
        );
        println!(
            "{}",
            format_row(
                "Recall @ 5",
                baseline_report.metrics.mean_recall_at_5,
                report.metrics.mean_recall_at_5
            )
        );
        println!(
            "{}",
            format_row(
                "Recall @ 10",
                baseline_report.metrics.mean_recall_at_10,
                report.metrics.mean_recall_at_10
            )
        );
        println!(
            "{}",
            format_row(
                "Precision @ 5",
                baseline_report.metrics.mean_precision_at_5,
                report.metrics.mean_precision_at_5
            )
        );
        println!(
            "{}",
            format_row(
                "Precision @ 10",
                baseline_report.metrics.mean_precision_at_10,
                report.metrics.mean_precision_at_10
            )
        );
        println!(
            "{}",
            format_row(
                "MRR",
                baseline_report.metrics.mean_mrr,
                report.metrics.mean_mrr
            )
        );
        println!(
            "{}",
            format_row(
                "nDCG @ 5",
                baseline_report.metrics.mean_ndcg_at_5,
                report.metrics.mean_ndcg_at_5
            )
        );
        println!(
            "{}",
            format_row(
                "nDCG @ 10",
                baseline_report.metrics.mean_ndcg_at_10,
                report.metrics.mean_ndcg_at_10
            )
        );
        println!("===========================");

        if let Err(regressions) = check_regressions(&report, &baseline_report) {
            eprintln!("\n[ERROR] Regression detected in quality metrics:");
            for reg in regressions {
                eprintln!("  - {}", reg);
            }
            std::process::exit(1);
        } else {
            println!("\n[SUCCESS] No quality metrics regressions detected.");
        }
    }
}
