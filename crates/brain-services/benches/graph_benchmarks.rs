/// Graph Capability Benchmark Suite — v0.8
///
/// Covers four benchmark categories introduced in v0.8:
///
/// 1. **Graph-aware retrieval** — `LtmMemorySource` with varying `graph_depth`
///    (0, 1, 2, 3) and graph sizes (100, 1 000, 10 000 nodes).
///
/// 2. **Relationship expansion** — `RelationshipExpander` over hub-and-spoke
///    topologies at sizes 10 → 1 000 spoke nodes.
///
/// 3. **Graph projectors** — `NeighborhoodProjector`, `PathProjector`, and
///    `ClusterProjector` over in-memory `KnowledgeGraph` at sizes 100 → 100 000
///    nodes.
///
/// 4. **Temporal projection** — `TemporalProjector::project_graph` under both
///    `Current` and `Historical` visibility modes at 100 → 10 000 temporal edges.
///
/// # Running
///
/// ```
/// cargo bench -p brain-services --bench graph_benchmarks
/// ```
///
/// # Output
///
/// Criterion writes HTML reports to `target/criterion/`.
/// A machine-readable JSON telemetry file is written to
/// `docs/benchmarks/graph_bench_telemetry.json`.
///
/// # Architectural invariants
///
/// These benchmarks are **read-only**: no production logic, API, or behaviour is
/// modified.  The `TrackingAllocator` is local to this binary; it does not
/// interfere with any other crate or test binary.
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use brain_core::events::CorrelationId;
use brain_core::projection::{ProjectionContext, Projector};
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{MemorySource, RetrievalRequest};
use brain_domain::{
    temporal::{
        RecencyPolicy, TemporalEdge, TemporalProjector, TemporalQuery, TemporalValidity,
        TemporalVisibility, TimeInterval, TimePoint,
    },
    Edge, EpochId, KnowledgeGraph, Node, NodeId, NodeType, RelationKind, RelationRegistry,
    SessionId,
};
use brain_services::graph::projections::{
    ClusterProjector, ClusterQuery, NeighborhoodProjector, NeighborhoodQuery, PathProjector,
    PathQuery,
};
use brain_services::retrieval::relationship_expander::RelationshipExpander;
use brain_services::retrieval::source::LtmMemorySource;
use brain_storage::TestStorage;

// ============================================================================
// Tracking Allocator
//
// Used for per-benchmark allocation accounting in the telemetry pass.
// Each `[[bench]]` target compiles into its own binary so there is no conflict
// with the global allocator in other benchmark files.
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
// Environment helpers (for telemetry JSON)
// ============================================================================

fn get_git_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
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
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown macos cpu".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("sh")
            .arg("-c")
            .arg("grep -m1 'model name' /proc/cpuinfo | cut -d: -f2")
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
// Synthetic graph builders — in-memory
// ============================================================================

/// Builds a chain graph with `count` nodes: n[0] → n[1] → … → n[count-1].
///
/// Used for `NeighborhoodProjector` and `PathProjector` benchmarks.  All edges
/// use `RelationKind::Uses` with weight 1.0.
fn build_chain_graph(count: usize) -> (KnowledgeGraph, Vec<NodeId>) {
    let mut graph = KnowledgeGraph::new();
    let ids: Vec<NodeId> = (0..count).map(|_| NodeId::new()).collect();

    for (i, &id) in ids.iter().enumerate() {
        graph.add_node(Node::new(
            id,
            format!("chain_node_{}", i),
            NodeType::Concept,
        ));
    }

    for i in 0..count.saturating_sub(1) {
        let _ = graph.add_edge(Edge::new(ids[i], ids[i + 1], RelationKind::Uses, 1.0));
    }

    (graph, ids)
}

/// Builds a ring graph with `count` nodes: n[i] → n[(i+1) % count].
///
/// Used for `ClusterProjector` benchmarks where we want a single connected
/// component.
fn build_ring_graph(count: usize) -> (KnowledgeGraph, Vec<NodeId>) {
    assert!(count >= 2, "ring graph requires at least 2 nodes");
    let mut graph = KnowledgeGraph::new();
    let ids: Vec<NodeId> = (0..count).map(|_| NodeId::new()).collect();

    for (i, &id) in ids.iter().enumerate() {
        graph.add_node(Node::new(id, format!("ring_node_{}", i), NodeType::Concept));
    }

    for i in 0..count {
        let src = ids[i];
        let tgt = ids[(i + 1) % count];
        // Ignore duplicate-edge errors (can happen in degenerate cases)
        let _ = graph.add_edge(Edge::new(src, tgt, RelationKind::Uses, 1.0));
    }

    (graph, ids)
}

// ============================================================================
// Synthetic graph builders — SQLite-backed (for retrieval / expander benches)
// ============================================================================

/// Seeds a `TestStorage` in-memory SQLite database with a chain topology.
///
/// Node 0 gets label `"rust language benchmark node"` so FTS queries for "rust"
/// always resolve to exactly one direct hit.  Edges form a linear chain:
/// node[0] → node[1] → … → node[count-1].
///
/// Returns `(test_store, node_ids)` where `node_ids[0]` is the "rust" node.
fn seed_storage_chain(count: usize) -> (TestStorage, Vec<NodeId>) {
    let test_store = TestStorage::new();
    let store = test_store.store();

    let ids: Vec<NodeId> = (0..count).map(|_| NodeId::new()).collect();

    // Batch-insert nodes
    let nodes: Vec<Node> = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            let label = if i == 0 {
                "rust language benchmark node".to_string()
            } else {
                format!("benchmark graph node {}", i)
            };
            Node::new(id, label, NodeType::Concept)
        })
        .collect();
    store.nodes().save_batch(&nodes).expect("node batch insert");

    // Batch-insert chain edges: node[i] → node[i+1]
    if count > 1 {
        let edges: Vec<Edge> = ids
            .windows(2)
            .map(|w| Edge::new(w[0], w[1], RelationKind::Uses, 1.0))
            .collect();
        store.edges().save_batch(&edges).expect("edge batch insert");
    }

    (test_store, ids)
}

/// Seeds a hub-and-spoke topology for `RelationshipExpander` benchmarks.
///
/// One hub node is connected to `spoke_count` spoke nodes via outgoing `Uses`
/// edges.  The expander is invoked for **all** `spoke_count + 1` nodes, so the
/// measured cost includes both the hub (many outgoing edges) and the spokes
/// (one incoming edge each).
///
/// Returns `(test_store, hub_id, spoke_ids)`.
fn seed_storage_hub(spoke_count: usize) -> (TestStorage, NodeId, Vec<NodeId>) {
    let test_store = TestStorage::new();
    let store = test_store.store();

    let hub_id = NodeId::new();
    let spoke_ids: Vec<NodeId> = (0..spoke_count).map(|_| NodeId::new()).collect();

    let hub_node = Node::new(hub_id, "hub node".to_string(), NodeType::Concept);
    store.nodes().save(&hub_node).expect("hub node insert");

    let spoke_nodes: Vec<Node> = spoke_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| Node::new(id, format!("spoke node {}", i), NodeType::Concept))
        .collect();
    store
        .nodes()
        .save_batch(&spoke_nodes)
        .expect("spoke batch insert");

    // Hub → each spoke
    let edges: Vec<Edge> = spoke_ids
        .iter()
        .map(|&sid| Edge::new(hub_id, sid, RelationKind::Uses, 1.0))
        .collect();
    store.edges().save_batch(&edges).expect("edge batch insert");

    (test_store, hub_id, spoke_ids)
}

// ============================================================================
// Temporal projection fixtures
// ============================================================================

/// Builds `edge_count` temporal edges distributed across three time windows.
///
/// Time layout (Unix seconds):
/// - Window A [10, 30): edges 0..edge_count/3
/// - Window B [30, 60): edges edge_count/3..2*edge_count/3
/// - Window C [60, 90): remaining edges
///
/// At reference time T=40 with `Current` visibility, only Window B edges are
/// active.  With `Historical` visibility, Windows A and B are visible.
///
/// Returns `(graph, temporal_edges)`.  The graph contains all nodes needed as
/// endpoints.
fn build_temporal_scenario(edge_count: usize) -> (KnowledgeGraph, Vec<TemporalEdge>) {
    let mut graph = KnowledgeGraph::new();
    let mut temporal_edges: Vec<TemporalEdge> = Vec::with_capacity(edge_count);

    let a = edge_count / 3;
    let b = 2 * edge_count / 3;

    for i in 0..edge_count {
        let src_id = NodeId::new();
        let tgt_id = NodeId::new();
        graph.add_node(Node::new(
            src_id,
            format!("temporal_src_{}", i),
            NodeType::Concept,
        ));
        graph.add_node(Node::new(
            tgt_id,
            format!("temporal_tgt_{}", i),
            NodeType::Concept,
        ));
        // Add edge to graph so project_graph can include it
        let _ = graph.add_edge(Edge::new(src_id, tgt_id, RelationKind::Uses, 1.0));

        // Assign windows deterministically
        let (start_s, end_s, observed_s): (u64, u64, u64) = if i < a {
            (10, 30, 5)
        } else if i < b {
            (30, 60, 25)
        } else {
            (60, 90, 55)
        };

        let interval = TimeInterval::new(
            TimePoint::from_unix_seconds(start_s),
            Some(TimePoint::from_unix_seconds(end_s)),
        )
        .unwrap();

        temporal_edges.push(TemporalEdge {
            edge: Edge::new(src_id, tgt_id, RelationKind::Uses, 1.0),
            validity: TemporalValidity::new(vec![interval]),
            observed_at: TimePoint::from_unix_seconds(observed_s),
        });
    }

    (graph, temporal_edges)
}

// ============================================================================
// Projection context helper
// ============================================================================

fn projection_context<'a, Q: brain_core::projection::ProjectionQuery>(
    graph: &'a KnowledgeGraph,
    query: &'a Q,
) -> ProjectionContext<'a, Q> {
    ProjectionContext {
        graph,
        epoch: EpochId(1),
        query,
        correlation_id: CorrelationId::new_v4(),
    }
}

// ============================================================================
// Retrieval request builder
// ============================================================================

fn bench_request(query: &str, graph_depth: Option<usize>) -> RetrievalRequest {
    RetrievalRequest {
        session_id: SessionId::new(),
        query: query.to_string(),
        limit: 50,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth,
        expand_relations: false,
        reference_time: None,
    }
}

// ============================================================================
// 1. Graph-Aware Retrieval Benchmarks
//
// Benchmark `LtmMemorySource::retrieve()` across graph depths and graph sizes.
// A chain topology ensures that increasing depth linearly expands the candidate
// set, making the depth–latency relationship straightforward to measure.
//
// Benchmark ID format: `graph_retrieval/depth_{d}_size_{n}`
// ============================================================================
fn bench_graph_retrieval(c: &mut Criterion) {
    let graph_sizes: &[usize] = &[100, 1_000, 10_000];
    let depths: &[usize] = &[0, 1, 2, 3];
    let registry = Arc::new(RelationRegistry::default_embedded());

    let mut group = c.benchmark_group("graph_retrieval");
    // Fewer criterion samples because each iteration hits SQLite
    group.sample_size(20);

    for &size in graph_sizes {
        // Seed once; share across all depth variants for this size
        let (test_store, _ids) = seed_storage_chain(size);
        let src = LtmMemorySource::new(test_store.store(), registry.clone());

        for &depth in depths {
            let request = bench_request("rust", Some(depth));
            let bench_id = BenchmarkId::from_parameter(format!("depth_{}_size_{}", depth, size));

            group.bench_with_input(bench_id, &depth, |b, _| {
                b.iter(|| black_box(src.retrieve(black_box(&request)).unwrap()))
            });
        }
    }

    group.finish();
}

// ============================================================================
// 2. Relationship Expansion Benchmarks
//
// Benchmark `RelationshipExpander::expand()` for hub-and-spoke topologies
// of increasing spoke counts.
//
// Benchmark ID format: `relationship_expansion/spokes_{n}`
// ============================================================================
fn bench_relationship_expansion(c: &mut Criterion) {
    let spoke_counts: &[usize] = &[10, 50, 100, 500, 1_000];

    let mut group = c.benchmark_group("relationship_expansion");
    group.sample_size(30);

    for &count in spoke_counts {
        let (test_store, hub_id, spoke_ids) = seed_storage_hub(count);
        let expander = RelationshipExpander::new(test_store.store());

        // Collect the full node set to expand (hub + all spokes)
        let mut nodes: Vec<Node> = Vec::with_capacity(count + 1);
        nodes.push(Node::new(hub_id, "hub".to_string(), NodeType::Concept));
        for &sid in &spoke_ids {
            nodes.push(Node::new(sid, "spoke".to_string(), NodeType::Concept));
        }
        let nodes = Arc::new(nodes);

        let bench_id = BenchmarkId::from_parameter(format!("spokes_{}", count));

        group.bench_with_input(bench_id, &count, |b, _| {
            let nodes_ref = nodes.clone();
            b.iter(|| black_box(expander.expand(black_box(nodes_ref.as_slice())).unwrap()))
        });
    }

    group.finish();
}

// ============================================================================
// 3a. Neighborhood Projector Benchmarks
//
// Benchmark `NeighborhoodProjector::project()` over chain graphs with varying
// graph size and BFS depth.  All data is in-memory; no SQLite overhead.
//
// Benchmark ID format: `neighborhood_projector/depth_{d}_size_{n}`
// ============================================================================
fn bench_neighborhood_projector(c: &mut Criterion) {
    let graph_sizes: &[usize] = &[100, 1_000, 10_000, 100_000];
    let depths: &[usize] = &[1, 2, 3];
    let projector = NeighborhoodProjector;

    let mut group = c.benchmark_group("neighborhood_projector");
    group.sample_size(50);

    for &size in graph_sizes {
        let (graph, ids) = build_chain_graph(size);
        let center = ids[0];

        for &depth in depths {
            let query = NeighborhoodQuery {
                center_node_id: center,
                depth,
            };
            let bench_id = BenchmarkId::from_parameter(format!("depth_{}_size_{}", depth, size));

            group.bench_with_input(bench_id, &depth, |b, _| {
                b.iter(|| {
                    let ctx = projection_context(&graph, &query);
                    black_box(projector.project(black_box(&ctx)))
                })
            });
        }
    }

    group.finish();
}

// ============================================================================
// 3b. Path Projector Benchmarks
//
// Benchmark `PathProjector::project()` for shortest-path queries over chains
// of increasing length.  The path length equals the graph size, giving O(n)
// BFS work.
//
// Benchmark ID format: `path_projector/size_{n}`
// ============================================================================
fn bench_path_projector(c: &mut Criterion) {
    let graph_sizes: &[usize] = &[100, 1_000, 10_000, 100_000];
    let projector = PathProjector;

    let mut group = c.benchmark_group("path_projector");
    group.sample_size(50);

    for &size in graph_sizes {
        let (graph, ids) = build_chain_graph(size);
        let source = ids[0];
        let target = ids[size - 1]; // Longest possible path

        let query = PathQuery {
            source_node_id: source,
            target_node_id: target,
        };
        let bench_id = BenchmarkId::from_parameter(format!("size_{}", size));

        group.bench_with_input(bench_id, &size, |b, _| {
            b.iter(|| {
                let ctx = projection_context(&graph, &query);
                black_box(projector.project(black_box(&ctx)))
            })
        });
    }

    group.finish();
}

// ============================================================================
// 3c. Cluster Projector Benchmarks
//
// Benchmark `ClusterProjector::project()` (connected-components via BFS) over
// ring graphs of increasing size.  A ring forms a single cluster, exercising
// the full graph traversal path.
//
// Benchmark ID format: `cluster_projector/size_{n}`
// ============================================================================
fn bench_cluster_projector(c: &mut Criterion) {
    let graph_sizes: &[usize] = &[100, 1_000, 10_000, 100_000];
    let projector = ClusterProjector;

    let mut group = c.benchmark_group("cluster_projector");
    group.sample_size(50);

    for &size in graph_sizes {
        let (graph, _ids) = build_ring_graph(size);
        let query = ClusterQuery {
            min_cluster_size: None,
        };
        let bench_id = BenchmarkId::from_parameter(format!("size_{}", size));

        group.bench_with_input(bench_id, &size, |b, _| {
            b.iter(|| {
                let ctx = projection_context(&graph, &query);
                black_box(projector.project(black_box(&ctx)))
            })
        });
    }

    group.finish();
}

// ============================================================================
// 4. Temporal Projection Benchmarks
//
// Benchmark `TemporalProjector::project_graph()` under `Current` and
// `Historical` visibility modes.  Edge counts are varied from 100 → 10 000.
//
// Benchmark ID format:
//   `temporal_projection/current_edges_{n}`
//   `temporal_projection/historical_edges_{n}`
// ============================================================================
fn bench_temporal_projection(c: &mut Criterion) {
    let edge_counts: &[usize] = &[100, 1_000, 10_000];

    let mut group = c.benchmark_group("temporal_projection");
    group.sample_size(50);

    // Reference time T=40 sits inside Window B [30, 60)
    let reference_time = TimePoint::from_unix_seconds(40);

    for &count in edge_counts {
        let (graph, temporal_edges) = build_temporal_scenario(count);

        // Current visibility: only edges valid exactly at T=40
        let current_query = TemporalQuery {
            reference_time,
            visibility: TemporalVisibility::Current,
            recency_policy: RecencyPolicy::None,
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("current_edges_{}", count)),
            &count,
            |b, _| {
                b.iter(|| {
                    black_box(TemporalProjector::project_graph(
                        black_box(&graph),
                        black_box(&temporal_edges),
                        black_box(&current_query),
                    ))
                })
            },
        );

        // Historical visibility: all edges ever observed at or before T=40
        let historical_query = TemporalQuery {
            reference_time,
            visibility: TemporalVisibility::Historical,
            recency_policy: RecencyPolicy::None,
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("historical_edges_{}", count)),
            &count,
            |b, _| {
                b.iter(|| {
                    black_box(TemporalProjector::project_graph(
                        black_box(&graph),
                        black_box(&temporal_edges),
                        black_box(&historical_query),
                    ))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Telemetry — single-run allocation and candidate counts
//
// Records per-operation metrics for the JSON report: heap bytes allocated,
// allocation count, and result candidate count.  These complement Criterion's
// latency percentiles.
// ============================================================================
fn collect_telemetry() -> serde_json::Value {
    let mut runs: Vec<serde_json::Value> = Vec::new();

    // ── 1. Graph retrieval telemetry ────────────────────────────────────────
    {
        let registry = Arc::new(RelationRegistry::default_embedded());
        let graph_sizes: &[usize] = &[100, 1_000, 10_000];
        let depths: &[usize] = &[0, 1, 2, 3];

        for &size in graph_sizes {
            let (test_store, _ids) = seed_storage_chain(size);
            let src = LtmMemorySource::new(test_store.store(), registry.clone());

            for &depth in depths {
                let request = bench_request("rust", Some(depth));

                ALLOC.reset();
                let t0 = Instant::now();
                let result = src.retrieve(&request).unwrap();
                let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;

                let alloc_bytes = ALLOC.allocated.load(Ordering::SeqCst);
                let alloc_count = ALLOC.allocations.load(Ordering::SeqCst);
                let candidate_count = result.nodes.len();

                runs.push(serde_json::json!({
                    "benchmark": "graph_retrieval",
                    "graph_size": size,
                    "graph_depth": depth,
                    "candidate_count": candidate_count,
                    "single_run_ms": elapsed_ms,
                    "heap_allocated_bytes": alloc_bytes,
                    "heap_allocation_count": alloc_count,
                }));
            }
        }
    }

    // ── 2. Relationship expansion telemetry ─────────────────────────────────
    {
        let spoke_counts: &[usize] = &[10, 50, 100, 500, 1_000];

        for &count in spoke_counts {
            let (test_store, hub_id, spoke_ids) = seed_storage_hub(count);
            let expander = RelationshipExpander::new(test_store.store());

            let mut nodes: Vec<Node> = Vec::with_capacity(count + 1);
            nodes.push(Node::new(hub_id, "hub".to_string(), NodeType::Concept));
            for &sid in &spoke_ids {
                nodes.push(Node::new(sid, "spoke".to_string(), NodeType::Concept));
            }

            ALLOC.reset();
            let t0 = Instant::now();
            let result = expander.expand(&nodes).unwrap();
            let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;

            let alloc_bytes = ALLOC.allocated.load(Ordering::SeqCst);
            let alloc_count = ALLOC.allocations.load(Ordering::SeqCst);

            runs.push(serde_json::json!({
                "benchmark": "relationship_expansion",
                "node_count": count + 1,
                "spoke_count": count,
                "expansion_count": result.len(),
                "single_run_ms": elapsed_ms,
                "heap_allocated_bytes": alloc_bytes,
                "heap_allocation_count": alloc_count,
            }));
        }
    }

    // ── 3. Graph projector telemetry ────────────────────────────────────────
    {
        let graph_sizes: &[usize] = &[100, 1_000, 10_000, 100_000];
        let projector = NeighborhoodProjector;

        for &size in graph_sizes {
            let (graph, ids) = build_chain_graph(size);
            let center = ids[0];
            let query = NeighborhoodQuery {
                center_node_id: center,
                depth: 2,
            };
            let ctx = projection_context(&graph, &query);

            ALLOC.reset();
            let t0 = Instant::now();
            let result = projector.project(&ctx);
            let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;

            let alloc_bytes = ALLOC.allocated.load(Ordering::SeqCst);
            let alloc_count = ALLOC.allocations.load(Ordering::SeqCst);

            runs.push(serde_json::json!({
                "benchmark": "neighborhood_projector",
                "graph_size": size,
                "depth": 2,
                "result_nodes": result.nodes.len(),
                "result_edges": result.edges.len(),
                "single_run_ms": elapsed_ms,
                "heap_allocated_bytes": alloc_bytes,
                "heap_allocation_count": alloc_count,
            }));
        }
    }

    // ── 4. Temporal projection telemetry ────────────────────────────────────
    {
        let edge_counts: &[usize] = &[100, 1_000, 10_000];
        let reference_time = TimePoint::from_unix_seconds(40);
        let current_query = TemporalQuery {
            reference_time,
            visibility: TemporalVisibility::Current,
            recency_policy: RecencyPolicy::None,
        };

        for &count in edge_counts {
            let (graph, temporal_edges) = build_temporal_scenario(count);

            ALLOC.reset();
            let t0 = Instant::now();
            let projected =
                TemporalProjector::project_graph(&graph, &temporal_edges, &current_query);
            let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;

            let alloc_bytes = ALLOC.allocated.load(Ordering::SeqCst);
            let alloc_count = ALLOC.allocations.load(Ordering::SeqCst);

            runs.push(serde_json::json!({
                "benchmark": "temporal_projection_current",
                "temporal_edge_count": count,
                "projected_nodes": projected.nodes.len(),
                "projected_edges": projected.edges.len(),
                "single_run_ms": elapsed_ms,
                "heap_allocated_bytes": alloc_bytes,
                "heap_allocation_count": alloc_count,
            }));
        }
    }

    serde_json::json!({
        "metadata": {
            "git_commit": get_git_commit(),
            "rust_version": get_rust_version(),
            "cpu_info": get_cpu_info(),
            "os": std::env::consts::OS,
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        },
        "runs": runs,
    })
}

// ============================================================================
// Top-level orchestrator: runs telemetry then delegates to criterion groups
// ============================================================================
fn bench_graph_capabilities(c: &mut Criterion) {
    // Collect allocation + candidate telemetry first (single warm pass)
    let telemetry = collect_telemetry();
    let report_path = std::path::PathBuf::from("docs/benchmarks/graph_bench_telemetry.json");
    if let (Some(parent), Ok(json_str)) = (
        report_path.parent(),
        serde_json::to_string_pretty(&telemetry),
    ) {
        let _ = std::fs::create_dir_all(parent);
        let _ = std::fs::write(&report_path, json_str);
    }

    // Run all criterion groups
    bench_graph_retrieval(c);
    bench_relationship_expansion(c);
    bench_neighborhood_projector(c);
    bench_path_projector(c);
    bench_cluster_projector(c);
    bench_temporal_projection(c);
}

criterion_group!(benches, bench_graph_capabilities);
criterion_main!(benches);
