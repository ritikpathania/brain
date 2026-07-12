use brain_domain::{
    KnowledgeGraph, Node, NodeId, NodeType, Edge, RelationKind,
    GraphAnalyticsContext, RelationRegistry
};
use brain_domain::retrieval::{
    RetrievalRequest, RetrievalPlanner, PlanOptimizer, RetrievalExecutor,
    ReciprocalRankFusion, NormalizedTieBreakerRanking, QueryCompiler,
    RetrievalExecutionContext, CostHeuristics,
    RetrievalEvent, RecordingSink, CompletionReason, CancellationChecker,
    NeverCancelled
};
use brain_services::retrieval::cache::ExecutionCache;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn create_test_graph() -> (KnowledgeGraph, NodeId, NodeId, NodeId) {
    let mut graph = KnowledgeGraph::new();
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();

    graph.add_node(Node::new(node_a, "Rust Programming".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "Concurrency Abstractions".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "Cooperative Cancellation".to_string(), NodeType::Concept));

    graph.add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 1.0)).unwrap();
    graph.add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 0.9)).unwrap();

    (graph, node_a, node_b, node_c)
}

struct TriggeredCancellation {
    triggered: Arc<AtomicBool>,
}

impl CancellationChecker for TriggeredCancellation {
    fn is_cancelled(&self) -> bool {
        self.triggered.load(Ordering::SeqCst)
    }
}

#[test]
fn test_cancellation_before_execution() {
    let (graph, _, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let request = RetrievalRequest {
        query: "Rust".to_string(),
        min_confidence: 0.5,
    };
    let compiler = QueryCompiler::new_default();
    let planner = RetrievalPlanner;
    let optimizer = PlanOptimizer;
    let plan = optimizer.optimize(planner.plan(&compiler.compile_legacy(&request).canonical_query), &CostHeuristics::default());

    // Always cancelled
    let checker = TriggeredCancellation {
        triggered: Arc::new(AtomicBool::new(true)),
    };

    let mut sink = RecordingSink::new();
    let result = executor.execute_stream(plan, &mut sink, &checker);

    // Verify terminal Cancelled event and empty results
    let events = sink.into_events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        RetrievalEvent::Completed { reason, .. } => {
            assert!(matches!(reason, CompletionReason::Cancelled));
        }
        _ => panic!("Expected terminal Cancelled event"),
    }
    assert!(result.candidates.is_empty());
}

#[test]
fn test_cancellation_during_retrieval() {
    let (graph, _, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let request = RetrievalRequest {
        query: "Rust".to_string(),
        min_confidence: 0.5,
    };
    let compiler = QueryCompiler::new_default();
    let planner = RetrievalPlanner;
    let optimizer = PlanOptimizer;
    let plan = optimizer.optimize(planner.plan(&compiler.compile_legacy(&request).canonical_query), &CostHeuristics::default());

    // Cancel during/after step execution (we toggle it dynamically)
    let triggered = Arc::new(AtomicBool::new(false));
    let triggered_clone = triggered.clone();

    // Custom sink that triggers cancellation on the first CandidateFound event
    struct CancelOnCandidateSink {
        inner: RecordingSink,
        triggered: Arc<AtomicBool>,
    }
    impl brain_domain::retrieval::stream::RetrievalSink for CancelOnCandidateSink {
        fn on_event(&mut self, event: RetrievalEvent) {
            if let RetrievalEvent::CandidateFound(_) = &event {
                self.triggered.store(true, Ordering::SeqCst);
            }
            self.inner.on_event(event);
        }
    }

    let mut sink = CancelOnCandidateSink {
        inner: RecordingSink::new(),
        triggered: triggered_clone,
    };
    let checker = TriggeredCancellation { triggered };

    let result = executor.execute_stream(plan, &mut sink, &checker);

    let events = sink.inner.into_events();
    // Verify we cancelled and exactly one Completed(Cancelled) event was emitted
    let mut cancel_count = 0;
    let mut finished_count = 0;
    for event in &events {
        if let RetrievalEvent::Completed { reason, .. } = event {
            match reason {
                CompletionReason::Cancelled => cancel_count += 1,
                CompletionReason::Finished => finished_count += 1,
            }
        }
    }
    assert_eq!(cancel_count, 1, "Must emit exactly one Cancelled event");
    assert_eq!(finished_count, 0, "Must not emit Finished event");
    assert!(result.candidates.is_empty(), "Should return empty candidates on cancellation");
}

#[test]
fn test_cancellation_cache_safety() {
    let (graph, _, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let cache = ExecutionCache::new();
    let compiler = QueryCompiler::new_default();
    let planner = RetrievalPlanner;
    let optimizer = PlanOptimizer;

    let request = RetrievalRequest {
        query: "Rust".to_string(),
        min_confidence: 0.5,
    };

    // 1. Populate Cache
    let mut sink1 = RecordingSink::new();
    let _ = cache.execute_cached(
        &context,
        &request,
        &CostHeuristics::default(),
        &compiler,
        &planner,
        &optimizer,
        &executor,
        &mut sink1,
        &NeverCancelled,
    );

    // 2. Query again with active cancellation
    let checker = TriggeredCancellation {
        triggered: Arc::new(AtomicBool::new(true)),
    };
    let mut sink2 = RecordingSink::new();
    let result = cache.execute_cached(
        &context,
        &request,
        &CostHeuristics::default(),
        &compiler,
        &planner,
        &optimizer,
        &executor,
        &mut sink2,
        &checker,
    );

    // Verify it terminated immediately without partial replays
    let events = sink2.into_events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        RetrievalEvent::Completed { reason, .. } => {
            assert!(matches!(reason, CompletionReason::Cancelled));
        }
        _ => panic!("Expected immediately Completed(Cancelled) event"),
    }
    assert!(result.candidates.is_empty());
}

#[test]
fn test_cancellation_post_completion_ignored() {
    let (graph, _, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let request = RetrievalRequest {
        query: "Rust".to_string(),
        min_confidence: 0.5,
    };
    let compiler = QueryCompiler::new_default();
    let planner = RetrievalPlanner;
    let optimizer = PlanOptimizer;
    let plan = optimizer.optimize(planner.plan(&compiler.compile_legacy(&request).canonical_query), &CostHeuristics::default());

    let triggered = Arc::new(AtomicBool::new(false));
    let checker = TriggeredCancellation { triggered: triggered.clone() };

    struct CancelAfterCompletionSink {
        inner: RecordingSink,
        triggered: Arc<AtomicBool>,
    }
    impl brain_domain::retrieval::stream::RetrievalSink for CancelAfterCompletionSink {
        fn on_event(&mut self, event: RetrievalEvent) {
            if let RetrievalEvent::Completed { .. } = &event {
                self.triggered.store(true, Ordering::SeqCst);
            }
            self.inner.on_event(event);
        }
    }

    let mut sink = CancelAfterCompletionSink {
        inner: RecordingSink::new(),
        triggered,
    };

    let result = executor.execute_stream(plan, &mut sink, &checker);

    // Result should be normal and successful (Finished)
    assert!(!result.candidates.is_empty());
    let events = sink.inner.into_events();
    let mut cancel_count = 0;
    let mut finished_count = 0;
    for event in &events {
        if let RetrievalEvent::Completed { reason, .. } = event {
            match reason {
                CompletionReason::Cancelled => cancel_count += 1,
                CompletionReason::Finished => finished_count += 1,
            }
        }
    }
    assert_eq!(finished_count, 1);
    assert_eq!(cancel_count, 0, "Cancellation after completion must be ignored");
}

#[test]
fn test_cancellation_transparency() {
    let (graph, _, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let request = RetrievalRequest {
        query: "Rust".to_string(),
        min_confidence: 0.5,
    };
    let compiler = QueryCompiler::new_default();
    let planner = RetrievalPlanner;
    let optimizer = PlanOptimizer;
    let plan = optimizer.optimize(planner.plan(&compiler.compile_legacy(&request).canonical_query), &CostHeuristics::default());

    // Legacy style mock executor execution (previously had no checker, now uses NeverCancelled)
    let result = executor.execute(plan.clone(), &NeverCancelled);
    assert!(!result.candidates.is_empty());

    let mut sink = RecordingSink::new();
    let stream_result = executor.execute_stream(plan, &mut sink, &NeverCancelled);

    assert_eq!(result.candidates, stream_result.candidates);
    assert_eq!(result.explanations, stream_result.explanations);
    
    // Ensure Single Completion is Finished
    let events = sink.into_events();
    let completed_event = events.iter().find(|e| matches!(e, RetrievalEvent::Completed { .. })).unwrap();
    if let RetrievalEvent::Completed { reason, .. } = completed_event {
        assert!(matches!(reason, CompletionReason::Finished));
    } else {
        panic!("Missing Completed event");
    }
}
