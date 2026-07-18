use brain_core::repositories::NodeRepository;
use brain_domain::{Node, NodeId, NodeType};
use brain_services::eval_harness::{
    compare_stable_reports, run_benchmark, BenchmarkReport, FtsRetriever, GroundTruthCorpus,
    RetrievalChannel,
};
use brain_storage::TestStorage;
use std::fs;
use std::path::Path;

#[test]
fn test_fts_benchmark_cold_and_warm_cache() {
    // 1. Initialize temporary test storage (runs migrations including Version 13)
    let test_storage = TestStorage::new();
    let sqlite = test_storage.storage();

    // 2. Load ground truth nodes
    let queries_json = include_str!("evaluation/queries.json");
    let ground_truth_json = include_str!("evaluation/ground_truth.json");
    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();

    // 3. Populate SQLite database with nodes
    for corpus_node in &ground_truth.nodes {
        let node_id = NodeId(uuid::Uuid::parse_str(&corpus_node.node_id).unwrap());
        let node = Node::new(node_id, corpus_node.content.clone(), NodeType::Concept);
        NodeRepository::save(sqlite, &node).unwrap();
    }

    // 4. Verify that DB triggers automatically populated FTS table
    let conn = sqlite.pool().get().unwrap();
    let count: usize = conn
        .query_row("SELECT COUNT(*) FROM node_search", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, ground_truth.nodes.len());

    // 5. Instantiate FtsRetriever
    let retriever = FtsRetriever::new(sqlite.pool().clone());

    // 6. Cold-Cache Benchmark Run
    conn.execute_batch("PRAGMA shrink_memory;").unwrap();
    let cold_report = run_benchmark(queries_json, ground_truth_json, &retriever, "cold").unwrap();

    // 7. Warm-Cache Benchmark Run
    for _ in 0..5 {
        let _ = run_benchmark(queries_json, ground_truth_json, &retriever, "warm").unwrap();
    }
    let warm_report = run_benchmark(queries_json, ground_truth_json, &retriever, "warm").unwrap();

    // 8. Handle regeneration vs validation
    let regenerate = std::env::var("REGENERATE_BASELINES").unwrap_or_default() == "1";

    if regenerate {
        println!("REGENERATE_BASELINES=1 detected. Writing golden retrieval baselines...");
        let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/evaluation");

        let cold_serialized = serde_json::to_string_pretty(&cold_report).unwrap();
        let warm_serialized = serde_json::to_string_pretty(&warm_report).unwrap();

        fs::write(base_path.join("baseline_cold.json"), cold_serialized).unwrap();
        fs::write(base_path.join("baseline_warm.json"), warm_serialized).unwrap();

        println!("Golden retrieval baselines successfully written.");
    } else {
        // Load baselines
        let baseline_cold_json = include_str!("evaluation/baseline_cold.json");
        let baseline_warm_json = include_str!("evaluation/baseline_warm.json");

        let baseline_cold: BenchmarkReport = serde_json::from_str(baseline_cold_json).unwrap();
        let baseline_warm: BenchmarkReport = serde_json::from_str(baseline_warm_json).unwrap();

        // Compare stable structures deterministically and report detailed metrics regressions
        if let Err(e) = compare_stable_reports(&cold_report.stable, &baseline_cold.stable) {
            panic!("Cold cache: {}", e);
        }
        if let Err(e) = compare_stable_reports(&warm_report.stable, &baseline_warm.stable) {
            panic!("Warm cache: {}", e);
        }
    }

    // Secondary checks to ensure retriever diagnostics invariants
    let q_001_res = warm_report
        .stable
        .query_results
        .iter()
        .find(|r| r.query_id == "q_001")
        .unwrap();
    assert_eq!(q_001_res.status, "success");
    assert_eq!(q_001_res.recall_at_1, 1.0);
    assert_eq!(q_001_res.mrr, 1.0);

    let q_001_diag = warm_report
        .measured
        .diagnostics
        .iter()
        .find(|d| d.query_id == "q_001")
        .unwrap();
    assert_eq!(
        q_001_diag.normalized_query,
        Some("how do i configure typescript client uds".to_string())
    );
    assert_eq!(
        q_001_diag.executed_query,
        Some("how OR do OR i OR configure OR typescript OR client OR uds".to_string())
    );
    assert!(!q_001_diag.candidates.is_empty());
    assert_eq!(
        q_001_diag.candidates[0].retrieval_channels,
        vec![RetrievalChannel::Fts]
    );
}
