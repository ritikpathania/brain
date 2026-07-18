use brain_domain::{
    AStar, AStarConfig, AnalyticsAlgorithm, Centrality, CentralityConfig, Closeness,
    ClosenessConfig, ConnectedComponents, ConnectedComponentsConfig, Connectivity,
    ConnectivityConfig, CycleDetectionConfig, CycleDetector, Distribution, DistributionConfig,
    Edge, GraphAnalyticsContext, KnowledgeGraph, Node, NodeId, NodeType, PageRank, PageRankConfig,
    ProvenanceConfig, ProvenanceStatistics, RelationKind, RoutingAlgorithm, SccConfig,
    ShortestPath, ShortestPathConfig, StronglyConnectedComponents, UniformWeightProvider,
    ZeroHeuristic,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn generate_cyclic_ring_graph(num_nodes: usize) -> (KnowledgeGraph, Vec<NodeId>) {
    let mut graph = KnowledgeGraph::new();
    let node_ids: Vec<NodeId> = (0..num_nodes).map(|_| NodeId::new()).collect();
    for (i, &id) in node_ids.iter().enumerate() {
        graph.add_node(Node::new(id, format!("Node{}", i), NodeType::Concept));
    }
    for i in 0..num_nodes {
        let target_idx = (i + 1) % num_nodes;
        graph
            .add_edge(Edge::new(
                node_ids[i],
                node_ids[target_idx],
                RelationKind::Uses,
                1.0,
            ))
            .unwrap();
    }
    (graph, node_ids)
}

fn bench_index_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("Index Construction");

    for size in &[10, 100, 1000, 10000] {
        let (graph, _) = generate_cyclic_ring_graph(*size);

        group.bench_with_input(BenchmarkId::new("context_new", size), size, |b, _| {
            b.iter(|| {
                GraphAnalyticsContext::new(&graph);
            });
        });

        group.bench_with_input(BenchmarkId::new("adjacency_build", size), size, |b, _| {
            b.iter(|| {
                let ctx = GraphAnalyticsContext::new(&graph);
                ctx.adjacency();
            });
        });

        group.bench_with_input(
            BenchmarkId::new("reverse_adjacency_build", size),
            size,
            |b, _| {
                b.iter(|| {
                    let ctx = GraphAnalyticsContext::new(&graph);
                    ctx.reverse_adjacency();
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("degrees_build", size), size, |b, _| {
            b.iter(|| {
                let ctx = GraphAnalyticsContext::new(&graph);
                ctx.degrees();
            });
        });
    }
    group.finish();
}

fn bench_solvers_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("Solver Execution (Reusing Context)");

    for size in &[10, 100, 1000, 10000] {
        let (graph, node_ids) = generate_cyclic_ring_graph(*size);
        let ctx = GraphAnalyticsContext::new(&graph);

        // Pre-initialize indices so we measure solver execution only
        ctx.adjacency();
        ctx.reverse_adjacency();
        ctx.degrees();

        group.bench_with_input(
            BenchmarkId::new("ConnectedComponents", size),
            size,
            |b, _| {
                b.iter(|| {
                    ConnectedComponents::new(&ctx, ConnectedComponentsConfig::default()).compute();
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("Centrality", size), size, |b, _| {
            b.iter(|| {
                Centrality::new(&ctx, CentralityConfig::default()).compute();
            });
        });

        group.bench_with_input(BenchmarkId::new("Distribution", size), size, |b, _| {
            b.iter(|| {
                Distribution::new(&ctx, DistributionConfig::default()).compute();
            });
        });

        group.bench_with_input(
            BenchmarkId::new("ProvenanceStatistics", size),
            size,
            |b, _| {
                b.iter(|| {
                    ProvenanceStatistics::new(&ctx, ProvenanceConfig::default()).compute();
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("ShortestPath", size), size, |b, _| {
            let src = node_ids[0];
            let dst = node_ids[size / 2];
            b.iter(|| {
                ShortestPath::new(&ctx, ShortestPathConfig::default(), UniformWeightProvider)
                    .compute(src, dst);
            });
        });

        // Limit CycleDetector benchmark to size <= 1000 to prevent long DFS times
        if *size <= 1000 {
            group.bench_with_input(BenchmarkId::new("CycleDetector", size), size, |b, _| {
                b.iter(|| {
                    CycleDetector::new(&ctx, CycleDetectionConfig::default()).compute();
                });
            });
        }

        // Limit PageRank to size <= 1000 because matrix power iterations on 10000 nodes are slow
        if *size <= 1000 {
            group.bench_with_input(BenchmarkId::new("PageRank", size), size, |b, _| {
                b.iter(|| {
                    PageRank::new(&ctx, PageRankConfig::default()).compute();
                });
            });
        }

        group.bench_with_input(
            BenchmarkId::new("StronglyConnectedComponents", size),
            size,
            |b, _| {
                b.iter(|| {
                    StronglyConnectedComponents::new(&ctx, SccConfig::default()).compute();
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("AStar", size), size, |b, _| {
            let src = node_ids[0];
            let dst = node_ids[size / 2];
            b.iter(|| {
                AStar::new(
                    &ctx,
                    AStarConfig::default(),
                    UniformWeightProvider,
                    ZeroHeuristic,
                )
                .compute(src, dst);
            });
        });

        if *size <= 100 {
            group.bench_with_input(
                BenchmarkId::new("ClosenessCentrality", size),
                size,
                |b, _| {
                    b.iter(|| {
                        Closeness::new(&ctx, ClosenessConfig::default(), UniformWeightProvider)
                            .compute();
                    });
                },
            );
        }

        group.bench_with_input(
            BenchmarkId::new("ConnectivityDiagnostics", size),
            size,
            |b, _| {
                b.iter(|| {
                    Connectivity::new(&ctx, ConnectivityConfig::default()).compute();
                });
            },
        );
    }
    group.finish();
}

fn bench_combined_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("Combined Execution (Construction + Solver)");

    for size in &[10, 100, 1000] {
        let (graph, _) = generate_cyclic_ring_graph(*size);

        group.bench_with_input(BenchmarkId::new("PageRank Full", size), size, |b, _| {
            b.iter(|| {
                let ctx = GraphAnalyticsContext::new(&graph);
                PageRank::new(&ctx, PageRankConfig::default()).compute();
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_index_construction,
    bench_solvers_execution,
    bench_combined_execution
);
criterion_main!(benches);
