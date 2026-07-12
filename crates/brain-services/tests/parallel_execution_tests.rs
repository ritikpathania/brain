use brain_domain::{
    KnowledgeGraph, Node, NodeId, NodeType, Edge, RelationKind,
    GraphAnalyticsContext, RelationRegistry
};
use brain_domain::retrieval::{
    RetrievalExecutor, ReciprocalRankFusion, NormalizedTieBreakerRanking,
    RetrievalExecutionContext, RetrievalEvent, RecordingSink, CompletionReason,
    CancellationChecker, NeverCancelled, PhysicalRetrievalPlan, PhysicalStep,
    VectorSource, KeywordSource, GraphExpansionSource,
    source::RetrievalSource, fusion::CandidateFusionStrategy, ranking::RankingStrategy
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn create_test_graph() -> (KnowledgeGraph, NodeId, NodeId, NodeId) {
    let mut graph = KnowledgeGraph::new();
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();

    graph.add_node(Node::new(node_a, "Rust Programming".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "Concurrent execution".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "Deterministic outcomes".to_string(), NodeType::Concept));

    graph.add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 1.0)).unwrap();
    graph.add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 0.9)).unwrap();

    (graph, node_a, node_b, node_c)
}

#[test]
fn test_parallel_determinism_invariant() {
    let (graph, _node_a, _node_b, _node_c) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    // Physical plan: Vector search + Keyword search + Dependent neighbors expansion
    let plan = PhysicalRetrievalPlan {
        physical_steps: vec![
            PhysicalStep::VectorRetrieve { query: "Rust".to_string() },
            PhysicalStep::KeywordRetrieve { query: "Concurrent".to_string() },
            PhysicalStep::ExpandNeighbors {
                source_nodes: vec![],
                policy: brain_domain::retrieval::ExpansionPolicy::default(),
            },
        ],
        cost: brain_domain::retrieval::EstimatedCost {
            vector_cost: 10.0,
            keyword_cost: 2.0,
            expansion_cost: 5.0,
            fusion_cost: 1.0,
            ranking_cost: 0.5,
        },
        heuristics_version: 1,
    };

    // 1. Run parallel executor stream
    let mut sink_parallel = RecordingSink::new();
    let result_parallel = executor.execute_stream(plan.clone(), &mut sink_parallel, &NeverCancelled);

    // 2. Perform manual sequential execution for reference
    let v_source = VectorSource::new("Rust".to_string());
    let k_source = KeywordSource::new("Concurrent".to_string());
    let v_candidates = v_source.retrieve(&context);
    let k_candidates = k_source.retrieve(&context);

    let mut seeds = Vec::new();
    for c in &v_candidates { seeds.push(c.node_id); }
    for c in &k_candidates { seeds.push(c.node_id); }

    let e_source = GraphExpansionSource::new(seeds, brain_domain::retrieval::ExpansionPolicy::default());
    let e_candidates = e_source.retrieve(&context);

    let runs = vec![v_candidates, k_candidates, e_candidates];
    let fusion = ReciprocalRankFusion::default();
    let ranking = NormalizedTieBreakerRanking;
    let fused = fusion.fuse(&runs);
    let (expected_candidates, expected_explanations) = ranking.rank(&fused);

    // Assert candidates and explanations match exactly
    assert_eq!(result_parallel.candidates, expected_candidates);
    assert_eq!(result_parallel.explanations, expected_explanations);

    // Verify events order in stream matches Vector -> Keyword -> Expansion -> Fusion -> Ranking
    let events = sink_parallel.into_events();
    let mut stages = Vec::new();
    for event in &events {
        if let RetrievalEvent::StageStarted { stage } = event {
            stages.push(*stage);
        }
    }
    use brain_domain::retrieval::RetrievalStage;
    assert_eq!(stages, vec![
        RetrievalStage::VectorSearch,
        RetrievalStage::KeywordSearch,
        RetrievalStage::GraphExpansion,
        RetrievalStage::Fusion,
        RetrievalStage::Ranking
    ]);
}

#[test]
fn test_variable_delay_determinism() {
    let (graph, _, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    // Vector has 50ms delay, Keyword has 5ms delay.
    // Keyword thread finishes way faster, but results must be sorted and merged in plan order.
    let plan = PhysicalRetrievalPlan {
        physical_steps: vec![
            PhysicalStep::VectorRetrieve { query: "Rust__delay_50ms".to_string() },
            PhysicalStep::KeywordRetrieve { query: "Concurrent__delay_5ms".to_string() },
        ],
        cost: brain_domain::retrieval::EstimatedCost {
            vector_cost: 10.0,
            keyword_cost: 2.0,
            expansion_cost: 0.0,
            fusion_cost: 1.0,
            ranking_cost: 0.5,
        },
        heuristics_version: 1,
    };

    let mut sink = RecordingSink::new();
    let result = executor.execute_stream(plan, &mut sink, &NeverCancelled);

    // Verify events are in the correct sequence (Vector first, then Keyword)
    let events = sink.into_events();
    let mut stages = Vec::new();
    for event in &events {
        if let RetrievalEvent::StageStarted { stage } = event {
            stages.push(*stage);
        }
    }
    use brain_domain::retrieval::RetrievalStage;
    assert_eq!(stages, vec![
        RetrievalStage::VectorSearch,
        RetrievalStage::KeywordSearch,
        RetrievalStage::Fusion,
        RetrievalStage::Ranking
    ]);
    assert!(!result.candidates.is_empty());
}

#[test]
fn test_high_repetition_stress() {
    let (graph, _, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let plan = PhysicalRetrievalPlan {
        physical_steps: vec![
            PhysicalStep::VectorRetrieve { query: "Rust Programming".to_string() },
            PhysicalStep::KeywordRetrieve { query: "Deterministic outcomes".to_string() },
        ],
        cost: brain_domain::retrieval::EstimatedCost {
            vector_cost: 10.0,
            keyword_cost: 2.0,
            expansion_cost: 0.0,
            fusion_cost: 1.0,
            ranking_cost: 0.5,
        },
        heuristics_version: 1,
    };

    // Run first baseline execution
    let mut sink_baseline = RecordingSink::new();
    let result_baseline = executor.execute_stream(plan.clone(), &mut sink_baseline, &NeverCancelled);
    let events_baseline = sink_baseline.into_events();

    // Run 1000 times and assert complete deterministic byte-for-byte identical output
    for _ in 0..1000 {
        let mut sink = RecordingSink::new();
        let result = executor.execute_stream(plan.clone(), &mut sink, &NeverCancelled);
        let events = sink.into_events();

        assert_eq!(result.candidates, result_baseline.candidates);
        assert_eq!(result.explanations, result_baseline.explanations);
        assert_eq!(result.report.planning.estimated_cost, result_baseline.report.planning.estimated_cost);
        assert_eq!(result.report.runtime.candidates_produced, result_baseline.report.runtime.candidates_produced);
        assert_eq!(result.report.runtime.candidates_fused, result_baseline.report.runtime.candidates_fused);
        assert_eq!(result.report.runtime.expansions_performed, result_baseline.report.runtime.expansions_performed);
        assert_eq!(result.report.runtime.ranking_operations, result_baseline.report.runtime.ranking_operations);

        assert_eq!(events.len(), events_baseline.len());
        for (e1, e2) in events.iter().zip(events_baseline.iter()) {
            match (e1, e2) {
                (RetrievalEvent::StageStarted { stage: s1 }, RetrievalEvent::StageStarted { stage: s2 }) => assert_eq!(s1, s2),
                (RetrievalEvent::StageCompleted { stage: s1 }, RetrievalEvent::StageCompleted { stage: s2 }) => assert_eq!(s1, s2),
                (RetrievalEvent::CandidateFound(c1), RetrievalEvent::CandidateFound(c2)) => {
                    assert_eq!(c1.node_id, c2.node_id);
                    assert_eq!(c1.source_id, c2.source_id);
                }
                (RetrievalEvent::ExplanationUpdated { node_id: n1, .. }, RetrievalEvent::ExplanationUpdated { node_id: n2, .. }) => assert_eq!(n1, n2),
                (RetrievalEvent::Completed { reason: r1, .. }, RetrievalEvent::Completed { reason: r2, .. }) => assert_eq!(r1, r2),
                _ => panic!("Event mismatch during high-repetition stress run"),
            }
        }
    }
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
fn test_parallel_cancellation_gating() {
    let (graph, _, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let plan = PhysicalRetrievalPlan {
        physical_steps: vec![
            PhysicalStep::VectorRetrieve { query: "Rust".to_string() },
            PhysicalStep::KeywordRetrieve { query: "Concurrent".to_string() },
        ],
        cost: brain_domain::retrieval::EstimatedCost {
            vector_cost: 10.0,
            keyword_cost: 2.0,
            expansion_cost: 0.0,
            fusion_cost: 1.0,
            ranking_cost: 0.5,
        },
        heuristics_version: 1,
    };

    // Pre-cancelled check
    let checker = TriggeredCancellation {
        triggered: Arc::new(AtomicBool::new(true)),
    };
    let mut sink = RecordingSink::new();
    let result = executor.execute_stream(plan, &mut sink, &checker);

    assert!(result.candidates.is_empty());
    let events = sink.into_events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        RetrievalEvent::Completed { reason, .. } => {
            assert!(matches!(reason, CompletionReason::Cancelled));
        }
        _ => panic!("Expected terminal Cancelled event"),
    }
}
