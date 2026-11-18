//! Large Persistent Corpus (10K+ nodes) Scaling and Latency Benchmark Suite (Phase 6)

use brain_core::repositories::NodeRepository;
use brain_domain::{Node, NodeId, NodeType};
use brain_storage::SqliteStorage;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

struct TempDbGuard {
    dir: PathBuf,
    _db_path: PathBuf,
}

impl Drop for TempDbGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn create_temp_db() -> (TempDbGuard, SqliteStorage) {
    let uuid = uuid::Uuid::new_v4().to_string();
    let dir = std::env::temp_dir().join(format!("bd-corpus-{}", &uuid[0..8]));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("corpus_10k.db");

    let storage = SqliteStorage::new(db_path.to_str().unwrap(), 4, true)
        .expect("Failed to initialize persistent SqliteStorage");
    (
        TempDbGuard {
            dir,
            _db_path: db_path,
        },
        storage,
    )
}

fn calculate_percentiles(mut durations_us: Vec<f64>) -> (f64, f64, f64) {
    durations_us.sort_by(|a, b| a.total_cmp(b));
    let n = durations_us.len();
    let p50 = durations_us[(n as f64 * 0.50) as usize];
    let p95 = durations_us[(n as f64 * 0.95) as usize];
    let p99 = durations_us[(n as f64 * 0.99) as usize];
    (p50, p95, p99)
}

#[test]
fn test_persistent_10k_corpus_ingestion_and_retrieval_scaling() {
    let (_guard, storage) = create_temp_db();

    println!("\n=== POPULATING 10,000 PERSISTED NODES IN SQLITE ===");
    let t_ingest_start = Instant::now();

    let batch_size = 1000;
    let total_nodes = 10_000;
    let mut node_ids = Vec::with_capacity(total_nodes);

    for batch_idx in 0..(total_nodes / batch_size) {
        let mut batch = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let global_idx = batch_idx * batch_size + i;
            let node_id = NodeId::new();
            node_ids.push(node_id);

            let node_type = match global_idx % 4 {
                0 => NodeType::Concept,
                1 => NodeType::Project,
                2 => NodeType::File,
                _ => NodeType::Person,
            };

            let label = format!(
                "Architecture Module #{:05} - Component {}",
                global_idx,
                global_idx % 50
            );
            let mut node = Node::new(node_id, label, node_type);
            node.properties.insert(
                "workspace_id".to_string(),
                serde_json::json!(format!("ws_{}", global_idx % 10)),
            );
            node.properties.insert(
                "author".to_string(),
                serde_json::json!(format!("engineer_{}", global_idx % 25)),
            );
            node.properties.insert(
                "version".to_string(),
                serde_json::json!(format!("1.{}.0", global_idx % 100)),
            );

            batch.push(node);
        }
        NodeRepository::save_batch(&storage, &batch).expect("Failed to batch-insert nodes");
    }

    let ingest_duration = t_ingest_start.elapsed();
    println!(
        "Ingested {} nodes in {:.2?} ({:.1} nodes/sec)",
        total_nodes,
        ingest_duration,
        total_nodes as f64 / ingest_duration.as_secs_f64()
    );

    // Verify count
    let all_nodes = NodeRepository::list_all(&storage).expect("Failed to list nodes");
    assert_eq!(all_nodes.len(), total_nodes);

    // 1. Cold query test
    let cold_target = node_ids[total_nodes / 2];
    let t_cold = Instant::now();
    let cold_res = NodeRepository::find_by_id(&storage, &cold_target)
        .unwrap()
        .unwrap();
    let cold_latency_us = t_cold.elapsed().as_micros() as f64;
    assert_eq!(cold_res.id, cold_target);

    // 2. Warm query latency benchmark across 200 queries
    let mut query_latencies_us = Vec::with_capacity(200);
    for i in 0..200 {
        let target_id = node_ids[(i * 47) % total_nodes];
        let t0 = Instant::now();
        let res = NodeRepository::find_by_id(&storage, &target_id)
            .unwrap()
            .unwrap();
        query_latencies_us.push(t0.elapsed().as_micros() as f64);
        assert_eq!(res.id, target_id);
    }

    let (p50, p95, p99) = calculate_percentiles(query_latencies_us);

    println!(
        "\n=== 10,000 CORPUS RETRIEVAL LATENCY ===\n\
         Cold Query:   {:>8.1} µs\n\
         Warm p50:     {:>8.1} µs\n\
         Warm p95:     {:>8.1} µs\n\
         Warm p99:     {:>8.1} µs",
        cold_latency_us, p50, p95, p99
    );

    // Assert p95 lookup latency on 10K database is < 5ms (5,000 µs)
    assert!(
        p95 < 5000.0,
        "p95 lookup latency ({:.1}µs) exceeded 5ms target on 10k corpus",
        p95
    );
    assert!(p50 <= p95 && p95 <= p99);
}
