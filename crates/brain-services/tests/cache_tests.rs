use brain_domain::{
    KnowledgeGraph, Node, NodeId, NodeType, Edge, RelationKind,
    GraphAnalyticsContext, RelationRegistry
};
use brain_domain::retrieval::{
    RetrievalRequest, RetrievalPlanner, PlanOptimizer, RetrievalExecutor,
    ReciprocalRankFusion, NormalizedTieBreakerRanking, QueryCompiler,
    RetrievalExecutionContext, QueryRequest,
    RetrievalEvent, RecordingSink, CostHeuristics, NeverCancelled
};
use brain_services::retrieval::cache::{SnapshotGenerator, ExecutionCache};
use brain_domain::retrieval::cache::CompiledQueryCacheKey;

fn create_test_graph() -> (KnowledgeGraph, NodeId, NodeId, NodeId) {
    let mut graph = KnowledgeGraph::new();
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();

    graph.add_node(Node::new(node_a, "Rust".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "Cargo package manager".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "Compiler tools".to_string(), NodeType::Concept));

    graph.add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 0.9)).unwrap();
    graph.add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 0.7)).unwrap();

    (graph, node_a, node_b, node_c)
}

#[test]
fn test_layered_execution_cache() {
    let (graph, _node_a, _node_b, _node_c) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let snapshot_gen = SnapshotGenerator::new();
    let snapshot_id = snapshot_gen.next_snapshot_id();
    let context = RetrievalExecutionContext::new(snapshot_id, &graph, &registry, &analytics, None);

    let compiler = QueryCompiler::new_default();
    let planner = RetrievalPlanner;
    let optimizer = PlanOptimizer;
    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let cache = ExecutionCache::new();

    let request = RetrievalRequest {
        query: "Rust".to_string(),
        min_confidence: 0.5,
    };

    // 1. Run uncached execution
    let mut sink_uncached = RecordingSink::new();
    let result_uncached = cache.execute_cached(
        &context,
        &request,
        &CostHeuristics::default(),
        &compiler,
        &planner,
        &optimizer,
        &executor,
        &mut sink_uncached,
        &NeverCancelled,
    );

    // Verify uncached stats (all misses)
    let stats1 = cache.stats();
    assert_eq!(stats1.compiled.misses, 1);
    assert_eq!(stats1.logical.misses, 1);
    assert_eq!(stats1.physical.misses, 1);
    assert_eq!(stats1.result.misses, 1);
    assert_eq!(stats1.aggregate.misses, 4);

    // 2. Run cached execution (should hit result cache)
    let mut sink_cached = RecordingSink::new();
    let result_cached = cache.execute_cached(
        &context,
        &request,
        &CostHeuristics::default(),
        &compiler,
        &planner,
        &optimizer,
        &executor,
        &mut sink_cached,
        &NeverCancelled,
    );

    // Verify Cache Transparency: identical results
    assert_eq!(result_uncached.candidates, result_cached.candidates);
    assert_eq!(result_uncached.explanations, result_cached.explanations);

    // Verify Cache Transparency: identical events, nesting, and completion reasons
    let events_uncached = sink_uncached.into_events();
    let events_cached = sink_cached.into_events();
    assert_eq!(events_uncached.len(), events_cached.len());
    for (ev_u, ev_c) in events_uncached.iter().zip(events_cached.iter()) {
        match (ev_u, ev_c) {
            (RetrievalEvent::StageStarted { stage: s1 }, RetrievalEvent::StageStarted { stage: s2 }) => {
                assert_eq!(s1, s2);
            }
            (RetrievalEvent::StageCompleted { stage: s1 }, RetrievalEvent::StageCompleted { stage: s2 }) => {
                assert_eq!(s1, s2);
            }
            (RetrievalEvent::CandidateFound(c1), RetrievalEvent::CandidateFound(c2)) => {
                assert_eq!(c1.node_id, c2.node_id);
                assert_eq!(c1.source_id, c2.source_id);
            }
            (RetrievalEvent::ExplanationUpdated { node_id: n1, .. }, RetrievalEvent::ExplanationUpdated { node_id: n2, .. }) => {
                assert_eq!(n1, n2);
            }
            (RetrievalEvent::Completed { reason: r1, .. }, RetrievalEvent::Completed { reason: r2, .. }) => {
                assert_eq!(r1, r2);
            }
            _ => panic!("Event mismatch under cached execution!"),
        }
    }

    // Verify stats showing result hit
    let stats2 = cache.stats();
    assert_eq!(stats2.compiled.hits, 0); // Result hit bypassed compiling
    assert_eq!(stats2.result.hits, 1);

    // 3. Test Invalidation
    cache.invalidate_snapshot(snapshot_id);
    let mut sink_invalidated = RecordingSink::new();
    let _ = cache.execute_cached(
        &context,
        &request,
        &CostHeuristics::default(),
        &compiler,
        &planner,
        &optimizer,
        &executor,
        &mut sink_invalidated,
        &NeverCancelled,
    );

    // Stats should record misses again
    let stats3 = cache.stats();
    assert_eq!(stats3.compiled.misses, 2);
    assert_eq!(stats3.result.misses, 2);

    // 4. Test Layer Independence (Partial Hits)
    // Clear cache, insert compiled query manually to force compiled hit but other misses
    cache.invalidate_snapshot(snapshot_id);
    let compiled_key = CompiledQueryCacheKey {
        snapshot_id,
        request: QueryRequest {
            semantic_query: request.query.clone(),
            min_confidence: request.min_confidence,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
        },
    };
    let compiled_val = compiler.compile_legacy(&request);
    cache.insert_compiled_query(compiled_key, compiled_val);

    let mut sink_partial = RecordingSink::new();
    let result_partial = cache.execute_cached(
        &context,
        &request,
        &CostHeuristics::default(),
        &compiler,
        &planner,
        &optimizer,
        &executor,
        &mut sink_partial,
        &NeverCancelled,
    );

    // Check compiled hit but others missed
    let stats4 = cache.stats();
    assert_eq!(stats4.compiled.hits, 1);
    assert_eq!(stats4.logical.misses, 3); // previous + this one
    assert_eq!(result_uncached.candidates, result_partial.candidates);
}
