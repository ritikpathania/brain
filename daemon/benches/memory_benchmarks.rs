use criterion::{criterion_group, criterion_main, Criterion};
use daemon_bridge::plugins::StorageBackend;
use daemon_bridge::stm;
use daemon_bridge::storage::duckdb::AnalyticsDatabase;
use daemon_bridge::storage::sqlite::LtmDatabase;
use daemon_bridge::storage::{ExtractedEdge, ExtractedNode};
use pyo3::prelude::*;

fn bench_stm_query(c: &mut Criterion) {
    let mut session = stm::SessionContext::new();
    session.ingest("setting up sqlite database configuration".to_string());
    session.ingest("storing API keys in environment variables".to_string());
    session.ingest("deploying node.js server to AWS environment".to_string());

    let mut group = c.benchmark_group("ShortTermMemory");
    group.bench_function("fuzzy_query_db_config", |b| {
        b.iter(|| {
            let _results = session.query("db config");
        })
    });
    group.finish();
}

fn bench_ltm_query(c: &mut Criterion) {
    let db = LtmDatabase::new_in_memory().unwrap();

    let node1 = ExtractedNode {
        id: "sqlite".to_string(),
        label: "SQLite".to_string(),
        node_type: "technology".to_string(),
        attributes: serde_json::json!({ "engine": "SQLite" }),
    };
    let node2 = ExtractedNode {
        id: "db-config".to_string(),
        label: "Database Configuration".to_string(),
        node_type: "configuration".to_string(),
        attributes: serde_json::json!({}),
    };
    let edge = ExtractedEdge {
        source: "db-config".to_string(),
        target: "sqlite".to_string(),
        relation: "configures".to_string(),
    };

    db.upsert_nodes_and_edges(&[node1, node2], &[edge]).unwrap();

    let mut group = c.benchmark_group("LongTermMemory");
    group.bench_function("sqlite_query_ltm", |b| {
        b.iter(|| {
            let _results = db.query_ltm("sqlite");
        })
    });
    group.finish();
}

fn bench_duckdb_sync(c: &mut Criterion) {
    let sqlite_db = LtmDatabase::new_in_memory().unwrap();
    let duck_db = AnalyticsDatabase::new_in_memory().unwrap();

    let node1 = ExtractedNode {
        id: "node-1".to_string(),
        label: "Label 1".to_string(),
        node_type: "type-a".to_string(),
        attributes: serde_json::json!({}),
    };
    sqlite_db.upsert_nodes_and_edges(&[node1], &[]).unwrap();

    let mut group = c.benchmark_group("Analytics");
    group.bench_function("incremental_sync_sqlite_to_duckdb", |b| {
        b.iter(|| {
            sqlite_db.with_connection(|conn| {
                duck_db.run_incremental_sync(conn).unwrap();
            });
        })
    });
    group.finish();
}

fn bench_hybrid_retrieval_pipeline(c: &mut Criterion) {
    let retrieval = daemon_bridge::retrieval::fuzzy::FuzzyRetrieval;
    let ranking = daemon_bridge::retrieval::reranker::DefaultRanking;

    let window = vec![stm::TempNode {
        id: "stm-node-1".to_string(),
        epoch: 1,
        content: "rust compiler optimization flags".to_string(),
        timestamp: 1000,
    }];
    let mut index = stm::STMIndex::new();
    index.add("stm-node-1".to_string(), "rust compiler optimization flags");

    let storage = LtmDatabase::new_in_memory().unwrap();

    let ltm_nodes = vec![ExtractedNode {
        id: "ltm-node-1".to_string(),
        label: "Rust Programming".to_string(),
        node_type: "language".to_string(),
        attributes: serde_json::json!({"level": "advanced"}),
    }];
    storage.upsert_nodes_and_edges(&ltm_nodes, &[]).unwrap();

    let mut group = c.benchmark_group("HybridRetrieval");
    group.bench_function("hybrid_pipeline_run", |b| {
        b.iter(|| {
            let _results = daemon_bridge::retrieval::pipeline::run_retrieval_pipeline(
                "rust compiler",
                &index,
                &window,
                &retrieval,
                &ranking,
                Some(&storage),
                None,
            )
            .unwrap();
        })
    });
    group.finish();
}

fn bench_ipc_parsing(c: &mut Criterion) {
    let legacy_json = r#"{"action":"ingest","payload":"settings configuration for sqlite"}"#;
    let versioned_json = r#"{"version":"1.0","type":"Request","id":100,"action":"ingest","body":"settings configuration for sqlite"}"#;

    let mut group = c.benchmark_group("IPCParsing");
    group.bench_function("parse_legacy_request", |b| {
        b.iter(|| {
            let _req: daemon_bridge::server::protocol::ClientRequest =
                serde_json::from_str(legacy_json).unwrap();
        })
    });
    group.bench_function("parse_versioned_request", |b| {
        b.iter(|| {
            let _req: daemon_bridge::server::protocol::ClientRequest =
                serde_json::from_str(versioned_json).unwrap();
        })
    });
    group.finish();
}

fn bench_vector_database(c: &mut Criterion) {
    let storage = LtmDatabase::new_in_memory().unwrap();

    let ltm_nodes = vec![
        ExtractedNode {
            id: "node-1".to_string(),
            label: "Node 1".to_string(),
            node_type: "concept".to_string(),
            attributes: serde_json::json!({}),
        },
        ExtractedNode {
            id: "node-2".to_string(),
            label: "Node 2".to_string(),
            node_type: "concept".to_string(),
            attributes: serde_json::json!({}),
        },
        ExtractedNode {
            id: "node-3".to_string(),
            label: "Node 3".to_string(),
            node_type: "concept".to_string(),
            attributes: serde_json::json!({}),
        },
    ];
    storage.upsert_nodes_and_edges(&ltm_nodes, &[]).unwrap();

    storage
        .write_embeddings(&[
            ("node-1".to_string(), vec![0.5; 384]),
            ("node-2".to_string(), vec![0.2; 384]),
            ("node-3".to_string(), vec![-0.1; 384]),
        ])
        .unwrap();

    let query_vec = vec![0.5; 384];
    let mut group = c.benchmark_group("VectorDatabase");
    group.bench_function("nearest_neighbors_query_384_dims", |b| {
        b.iter(|| {
            let _neighbors = storage.query_nearest_neighbors(&query_vec, 2).unwrap();
        })
    });
    group.finish();
}

fn bench_cold_startup(c: &mut Criterion) {
    let mut group = c.benchmark_group("ColdStartup");
    group.bench_function("db_initialization", |b| {
        b.iter(|| {
            let ltm = LtmDatabase::new_in_memory().unwrap();
            let duck = AnalyticsDatabase::new_in_memory().unwrap();
            (ltm, duck)
        })
    });
    group.finish();
}

fn bench_indexing_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("IndexingThroughput");
    group.bench_function("stm_ingestion_100_items", |b| {
        b.iter(|| {
            let mut session = stm::SessionContext::new();
            for i in 0..100 {
                session.ingest(format!("item number {}", i));
            }
            session
        })
    });
    group.finish();
}

fn bench_ffi_overhead(c: &mut Criterion) {
    pyo3::prepare_freethreaded_python();

    let mut group = c.benchmark_group("FFIOverhead");
    group.bench_function("rust_direct_call", |b| {
        b.iter(|| {
            let input = "hello world";
            let _output = format!("rust response: {}", input);
        })
    });
    group.bench_function("pyo3_python_ffi_call", |b| {
        b.iter(|| {
            pyo3::Python::with_gil(|py| {
                let input = "hello world";
                let py_str = pyo3::types::PyString::new_bound(py, input);
                let _output: String = py
                    .eval_bound(&format!("'python response: ' + '{}'", py_str), None, None)
                    .unwrap()
                    .extract()
                    .unwrap();
            });
        })
    });
    group.finish();
}

fn bench_memory_growth_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("MemoryGrowthSimulation");
    group.bench_function("allocate_and_store_1000_nodes", |b| {
        b.iter(|| {
            let mut nodes = Vec::with_capacity(1000);
            for i in 0..1000 {
                nodes.push(stm::TempNode {
                    id: format!("node-{}", i),
                    epoch: 1,
                    content: format!("This is memory test content for node index: {}", i),
                    timestamp: i,
                });
            }
            nodes
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_stm_query,
    bench_ltm_query,
    bench_duckdb_sync,
    bench_hybrid_retrieval_pipeline,
    bench_ipc_parsing,
    bench_vector_database,
    bench_cold_startup,
    bench_indexing_throughput,
    bench_ffi_overhead,
    bench_memory_growth_simulation
);
criterion_main!(benches);
