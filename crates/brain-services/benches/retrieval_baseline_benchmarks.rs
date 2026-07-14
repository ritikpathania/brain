use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::path::PathBuf;
use std::fs;

use brain_core::retrieval::{MemorySource, RetrievalRequest};
use brain_domain::RelationRegistry;
use brain_storage::store::SqliteStorage;

// ============================================================================
// 1. Isolated Tracking Allocator (No impact on production code)
// ============================================================================
struct TrackingAllocator {
    allocated: AtomicUsize,
    deallocated: AtomicUsize,
    allocations: AtomicUsize,
}

impl TrackingAllocator {
    const fn new() -> Self {
        Self {
            allocated: AtomicUsize::new(0),
            deallocated: AtomicUsize::new(0),
            allocations: AtomicUsize::new(0),
        }
    }

    fn reset(&self) {
        self.allocated.store(0, Ordering::SeqCst);
        self.deallocated.store(0, Ordering::SeqCst);
        self.allocations.store(0, Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            self.allocated.fetch_add(layout.size(), Ordering::SeqCst);
            self.allocations.fetch_add(1, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        self.deallocated.fetch_add(layout.size(), Ordering::SeqCst);
    }
}

#[global_allocator]
static ALLOC: TrackingAllocator = TrackingAllocator::new();

// ============================================================================
// 2. Deterministic Dataset Generators (Reproducible seed & loop)
// ============================================================================
fn deterministic_label(index: usize) -> String {
    let categories = [
        "concept",
        "server",
        "database",
        "assistant",
        "optimization",
        "configuration",
        "deployment",
        "compiler",
    ];
    let cat = categories[index % categories.len()];
    format!("Standard Node Label for {} item index {}", cat, index)
}

fn seed_database(storage: &SqliteStorage, count: usize) {
    let mut conn = storage.pool().get().unwrap();
    let tx = conn.transaction().unwrap();
    {
        let mut stmt = tx
            .prepare("INSERT INTO nodes (id, label, node_type, properties, updated_at) VALUES (?, ?, ?, ?, ?)")
            .unwrap();
        for i in 0..count {
            let mut bytes = [0u8; 16];
            let i_bytes = (i as u64).to_be_bytes();
            bytes[8..16].copy_from_slice(&i_bytes);
            let id = uuid::Uuid::from_bytes(bytes).to_string();
            let label = deterministic_label(i);
            let n_type = if i % 2 == 0 { brain_domain::NodeType::Concept } else { brain_domain::NodeType::Technology };
            let node_type_str = serde_json::to_string(&n_type).unwrap();
            let properties = serde_json::to_string(&serde_json::json!({
                "index": i,
                "importance": (i % 10) as f64
            }))
            .unwrap();
            let updated_at = 1000 + i as i64;
            stmt.execute(rusqlite::params![id, label, node_type_str, properties, updated_at])
                .unwrap();
        }
    }
    tx.commit().unwrap();
}

// ============================================================================
// 3. Environment Metadata & Exporter
// ============================================================================
fn get_git_commit() -> String {
    std::process::Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn get_rust_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn get_cpu_info() -> String {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sysctl")
            .args(&["-n", "machdep.cpu.brand_string"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown macos cpu".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("sh")
            .arg("-c")
            .arg("grep -m 1 'model name' /proc/cpuinfo | cut -d: -f2")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown linux cpu".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        "unknown cpu".to_string()
    }
}

// ============================================================================
// 4. Correctness Assertion & Telemetry Trace
// ============================================================================
fn run_baseline_tracing(storage: &SqliteStorage, count: usize) -> serde_json::Value {
    let source = brain_services::retrieval::source::LtmMemorySource::new(
        Arc::new(SqliteStorage::from_pool(storage.pool().clone())),
        Arc::new(RelationRegistry::default_embedded()),
    );

    let request = RetrievalRequest {
        session_id: brain_domain::SessionId::new(),
        query: "compiler".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    // Warmup
    let _ = source.retrieve(&request).unwrap();

    // Reset allocator and execute
    ALLOC.reset();
    let start_time = Instant::now();
    let result = source.retrieve(&request).unwrap();
    let duration = start_time.elapsed();
    let allocated_bytes = ALLOC.allocated.load(Ordering::SeqCst);
    let allocation_count = ALLOC.allocations.load(Ordering::SeqCst);

    // Assert Correctness: Fail early and immediately if parity/casing is wrong!
    assert!(
        !result.nodes.is_empty(),
        "Correctness Failure: query returned no nodes!"
    );
    let contains_keyword = result
        .nodes
        .iter()
        .any(|node| node.label.to_lowercase().contains("compiler"));
    assert!(
        contains_keyword,
        "Correctness Failure: result nodes do not contain query token 'compiler'!"
    );

    serde_json::json!({
        "scale_nodes": count,
        "correctness_verified": true,
        "single_query_duration_ms": duration.as_secs_f64() * 1000.0,
        "heap_allocated_bytes": allocated_bytes,
        "heap_allocation_count": allocation_count
    })
}

// ============================================================================
// 5. Benchmark Group Definition
// ============================================================================
fn bench_retrieval_baseline(c: &mut Criterion) {
    let scales = [100, 1000, 10000];
    let mut telemetry_results = Vec::new();

    for &count in &scales {
        // Setup temp database file
        let rand_val = uuid::Uuid::new_v4().to_string();
        let db_file = std::env::temp_dir().join(format!("bench-ltm-{}-{}.db", count, rand_val));
        let db_path = db_file.to_str().unwrap();

        let storage = SqliteStorage::new(db_path, 1, false).unwrap();
        seed_database(&storage, count);

        // 1. Run correctness & allocation tracing first (failing fast on incorrect behavior)
        let telemetry = run_baseline_tracing(&storage, count);
        telemetry_results.push(telemetry);

        // 2. Perform latency micro-benchmarks with Criterion
        let ltm_source = brain_services::retrieval::source::LtmMemorySource::new(
            Arc::new(SqliteStorage::from_pool(storage.pool().clone())),
            Arc::new(RelationRegistry::default_embedded()),
        );

        let request = RetrievalRequest {
            session_id: brain_domain::SessionId::new(),
            query: "compiler".to_string(),
            limit: 10,
            exclude_ids: std::collections::HashSet::new(),
            deadline: None,
        };

        let mut group = c.benchmark_group("LtmMemorySource_Baseline");
        group.bench_function(format!("retrieve_ltm_scale_{}", count), |b| {
            b.iter(|| {
                let _res = ltm_source.retrieve(black_box(&request)).unwrap();
            })
        });
        group.finish();

        // Cleanup
        let _ = fs::remove_file(db_file);
    }

    // Write machine-readable baseline to conversational artifact directory
    let artifact_dir = PathBuf::from("/Users/ritikpathania/.gemini/antigravity/brain/34b4c5e5-7d12-4562-abaf-12d79236c9cb");
    let results_path = artifact_dir.join("baseline_bench_results.json");

    let final_report = serde_json::json!({
        "metadata": {
            "git_commit": get_git_commit(),
            "rust_version": get_rust_version(),
            "cpu_info": get_cpu_info(),
            "os": std::env::consts::OS,
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "timestamp": chrono::Utc::now().to_rfc3339()
        },
        "runs": telemetry_results
    });

    if let Ok(json_str) = serde_json::to_string_pretty(&final_report) {
        let _ = fs::create_dir_all(&artifact_dir);
        let _ = fs::write(results_path, json_str);
    }
}

criterion_group!(benches, bench_retrieval_baseline);
criterion_main!(benches);
