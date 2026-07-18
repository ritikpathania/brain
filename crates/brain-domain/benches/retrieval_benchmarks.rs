use brain_domain::{
    CostHeuristics, Edge, ExpansionPolicy, GraphAnalyticsContext, KnowledgeGraph,
    LogicalRetrievalPlan, LogicalStep, NeverCancelled, Node, NodeId, NodeType,
    NormalizedTieBreakerRanking, PlanOptimizer, QueryCompiler, ReciprocalRankFusion, RelationKind,
    RelationRegistry, RetrievalExecutionContext, RetrievalExecutor, RetrievalPlanner,
    RetrievalRequest,
};
use criterion::{criterion_group, criterion_main, Criterion};

fn generate_benchmark_graph(size: usize) -> (KnowledgeGraph, Vec<NodeId>) {
    let mut graph = KnowledgeGraph::new();
    let node_ids: Vec<NodeId> = (0..size).map(|_| NodeId::new()).collect();
    for (i, &id) in node_ids.iter().enumerate() {
        let label = format!("Node_{}", i);
        graph.add_node(Node::new(id, label, NodeType::Concept));
    }
    for i in 0..size - 1 {
        graph
            .add_edge(Edge::new(
                node_ids[i],
                node_ids[i + 1],
                RelationKind::Uses,
                1.0,
            ))
            .unwrap();
    }
    (graph, node_ids)
}

fn bench_retrieval_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("Retrieval Strategies");
    let size = 100;
    let (graph, node_ids) = generate_benchmark_graph(size);
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(
        brain_domain::SnapshotId::new(1),
        &graph,
        &registry,
        &analytics,
        None,
    );

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let request = RetrievalRequest {
        query: "Node_50".to_string(),
        min_confidence: 0.0,
    };

    // 1. Semantic-only Plan
    let semantic_logical = LogicalRetrievalPlan {
        steps: vec![LogicalStep::VectorRetrieve {
            query: request.query.clone(),
        }],
    };
    let semantic_physical = PlanOptimizer.optimize(semantic_logical, &CostHeuristics::default());

    group.bench_function("Semantic Only", |b| {
        b.iter(|| {
            executor.execute(semantic_physical.clone(), &NeverCancelled);
        });
    });

    // 2. Keyword-only Plan
    let keyword_logical = LogicalRetrievalPlan {
        steps: vec![LogicalStep::KeywordRetrieve {
            query: request.query.clone(),
        }],
    };
    let keyword_physical = PlanOptimizer.optimize(keyword_logical, &CostHeuristics::default());

    group.bench_function("Keyword Only", |b| {
        b.iter(|| {
            executor.execute(keyword_physical.clone(), &NeverCancelled);
        });
    });

    // 3. Graph-only Plan (neighborhood expansion starting from Node_50)
    let graph_logical = LogicalRetrievalPlan {
        steps: vec![LogicalStep::ExpandNeighbors {
            source_nodes: vec![node_ids[50]],
            policy: ExpansionPolicy::default(),
        }],
    };
    let graph_physical = PlanOptimizer.optimize(graph_logical, &CostHeuristics::default());

    group.bench_function("Graph Expansion Only", |b| {
        b.iter(|| {
            executor.execute(graph_physical.clone(), &NeverCancelled);
        });
    });

    // 4. Hybrid Plan (all sources merged)
    let hybrid_logical = RetrievalPlanner.plan(
        &QueryCompiler::new_default()
            .compile_legacy(&request)
            .canonical_query,
    );
    let hybrid_physical = PlanOptimizer.optimize(hybrid_logical, &CostHeuristics::default());

    group.bench_function("Hybrid", |b| {
        b.iter(|| {
            executor.execute(hybrid_physical.clone(), &NeverCancelled);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_retrieval_strategies);
criterion_main!(benches);
