use brain_domain::{
    KnowledgeGraph, Node, NodeId, NodeType, Edge, RelationKind,
    GraphAnalyticsContext, RelationRegistry
};
use brain_domain::retrieval::{
    RetrievalExecutor, ReciprocalRankFusion, NormalizedTieBreakerRanking,
    RetrievalExecutionContext, RetrievalEvent, RecordingSink, CompletionReason,
    CancellationChecker, NeverCancelled, PhysicalRetrievalPlan, PhysicalStep,
    ExecutionPolicy, SpeculationPlan, SpeculationStrategy, CanonicalQuery
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

struct DummyStrategy {
    seeds: Vec<NodeId>,
}
impl SpeculationStrategy for DummyStrategy {
    fn predict(&self, _query: &CanonicalQuery, _context: &RetrievalExecutionContext) -> SpeculationPlan {
        SpeculationPlan {
            predicted_seeds: self.seeds.clone(),
            confidence: 0.99,
            reason: "DummyStrategy prediction".to_string(),
        }
    }
}

#[test]
fn test_builder_replacement_semantics() {
    let (graph, node_a, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    // 1. Verify baseline defaults
    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );
    assert_eq!(executor.policy, ExecutionPolicy::Parallel);

    // 2. Verify builder replacement of policy
    let executor = executor.with_policy(ExecutionPolicy::Sequential);
    assert_eq!(executor.policy, ExecutionPolicy::Sequential);
    let executor = executor.with_policy(ExecutionPolicy::Speculative);
    assert_eq!(executor.policy, ExecutionPolicy::Speculative);

    // 3. Verify builder replacement of strategy
    let strategy_a = DummyStrategy { seeds: vec![node_a] };
    let strategy_b = DummyStrategy { seeds: vec![] };

    let executor = executor.with_speculation_strategy(Box::new(strategy_a));
    // Test execution with speculation plan
    let plan = CanonicalQuery {
        semantic_query: "".to_string(),
        min_confidence: 0.0,
        entity_types: None,
        relations: None,
        max_visited: None,
        max_depth: None,
        disable_expansion: false,
    };
    let spec_plan = executor.speculation_strategy.predict(&plan, &context);
    assert_eq!(spec_plan.predicted_seeds.len(), 1);

    let executor = executor.with_speculation_strategy(Box::new(strategy_b));
    let spec_plan = executor.speculation_strategy.predict(&plan, &context);
    assert_eq!(spec_plan.predicted_seeds.len(), 0); // Replaced successfully
}

#[test]
fn test_speculation_transparency_and_hit() {
    let (graph, node_a, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let plan = PhysicalRetrievalPlan {
        physical_steps: vec![
            PhysicalStep::VectorRetrieve { query: "Rust".to_string() },
            PhysicalStep::ExpandNeighbors {
                source_nodes: vec![],
                policy: brain_domain::retrieval::ExpansionPolicy::default(),
            },
        ],
        cost: brain_domain::retrieval::EstimatedCost {
            vector_cost: 10.0,
            keyword_cost: 0.0,
            expansion_cost: 5.0,
            fusion_cost: 1.0,
            ranking_cost: 0.5,
        },
        heuristics_version: 1,
    };

    // Baseline: Sequential execution
    let seq_executor = RetrievalExecutor::new(&context, ReciprocalRankFusion::default(), NormalizedTieBreakerRanking)
        .with_policy(ExecutionPolicy::Sequential);
    let mut seq_sink = RecordingSink::new();
    let seq_result = seq_executor.execute_stream(plan.clone(), &mut seq_sink, &NeverCancelled);
    let seq_events = seq_sink.into_events();

    // Speculation Hit: Speculative execution with correct seeds
    let spec_hit_executor = RetrievalExecutor::new(&context, ReciprocalRankFusion::default(), NormalizedTieBreakerRanking)
        .with_policy(ExecutionPolicy::Speculative)
        .with_speculation_strategy(Box::new(DummyStrategy { seeds: vec![node_a] }));
    let mut hit_sink = RecordingSink::new();
    let hit_result = spec_hit_executor.execute_stream(plan.clone(), &mut hit_sink, &NeverCancelled);
    let hit_events = hit_sink.into_events();

    // Verify Speculation Transparency: Hit is identical to Sequential
    assert_eq!(hit_result.candidates, seq_result.candidates);
    assert_eq!(hit_result.explanations, seq_result.explanations);
    assert_eq!(hit_result.report.runtime.candidates_produced, seq_result.report.runtime.candidates_produced);
    assert_eq!(hit_result.report.runtime.expansions_performed, seq_result.report.runtime.expansions_performed);

    assert_eq!(hit_events.len(), seq_events.len());
    for (e1, e2) in hit_events.iter().zip(seq_events.iter()) {
        match (e1, e2) {
            (RetrievalEvent::StageStarted { stage: s1 }, RetrievalEvent::StageStarted { stage: s2 }) => assert_eq!(s1, s2),
            (RetrievalEvent::StageCompleted { stage: s1 }, RetrievalEvent::StageCompleted { stage: s2 }) => assert_eq!(s1, s2),
            (RetrievalEvent::CandidateFound(c1), RetrievalEvent::CandidateFound(c2)) => assert_eq!(c1.node_id, c2.node_id),
            (RetrievalEvent::Completed { reason: r1, .. }, RetrievalEvent::Completed { reason: r2, .. }) => assert_eq!(r1, r2),
            _ => {}
        }
    }
}

#[test]
fn test_speculation_miss_false_positive() {
    let (graph, _, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let plan = PhysicalRetrievalPlan {
        physical_steps: vec![
            PhysicalStep::VectorRetrieve { query: "Rust".to_string() },
            PhysicalStep::ExpandNeighbors {
                source_nodes: vec![],
                policy: brain_domain::retrieval::ExpansionPolicy::default(),
            },
        ],
        cost: brain_domain::retrieval::EstimatedCost {
            vector_cost: 10.0,
            keyword_cost: 0.0,
            expansion_cost: 5.0,
            fusion_cost: 1.0,
            ranking_cost: 0.5,
        },
        heuristics_version: 1,
    };

    // Sequential baseline
    let seq_executor = RetrievalExecutor::new(&context, ReciprocalRankFusion::default(), NormalizedTieBreakerRanking)
        .with_policy(ExecutionPolicy::Sequential);
    let mut seq_sink = RecordingSink::new();
    let seq_result = seq_executor.execute_stream(plan.clone(), &mut seq_sink, &NeverCancelled);
    let seq_events = seq_sink.into_events();

    // Speculation Miss: Speculative execution with incorrect/false-positive seed node
    let bad_seed = NodeId::new(); // Random node ID not in the graph
    let spec_miss_executor = RetrievalExecutor::new(&context, ReciprocalRankFusion::default(), NormalizedTieBreakerRanking)
        .with_policy(ExecutionPolicy::Speculative)
        .with_speculation_strategy(Box::new(DummyStrategy { seeds: vec![bad_seed] }));

    let mut miss_sink = RecordingSink::new();
    let miss_result = spec_miss_executor.execute_stream(plan.clone(), &mut miss_sink, &NeverCancelled);
    let miss_events = miss_sink.into_events();

    // Verify Speculation Transparency & Discard: Miss falls back correctly and is identical to Sequential
    assert_eq!(miss_result.candidates, seq_result.candidates);
    assert_eq!(miss_result.explanations, seq_result.explanations);
    assert_eq!(miss_result.report.runtime.candidates_produced, seq_result.report.runtime.candidates_produced);
    assert_eq!(miss_result.report.runtime.expansions_performed, seq_result.report.runtime.expansions_performed);

    // Assert that the discarded bad branch left zero trace in the event sequence
    assert_eq!(miss_events.len(), seq_events.len());
    for (e1, e2) in miss_events.iter().zip(seq_events.iter()) {
        match (e1, e2) {
            (RetrievalEvent::StageStarted { stage: s1 }, RetrievalEvent::StageStarted { stage: s2 }) => assert_eq!(s1, s2),
            (RetrievalEvent::StageCompleted { stage: s1 }, RetrievalEvent::StageCompleted { stage: s2 }) => assert_eq!(s1, s2),
            (RetrievalEvent::CandidateFound(c1), RetrievalEvent::CandidateFound(c2)) => assert_eq!(c1.node_id, c2.node_id),
            (RetrievalEvent::Completed { reason: r1, .. }, RetrievalEvent::Completed { reason: r2, .. }) => assert_eq!(r1, r2),
            _ => {}
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
fn test_speculative_cancellation_gating() {
    let (graph, node_a, _, _) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(brain_domain::SnapshotId::new(1), &graph, &registry, &analytics, None);

    let plan = PhysicalRetrievalPlan {
        physical_steps: vec![
            PhysicalStep::VectorRetrieve { query: "Rust".to_string() },
            PhysicalStep::ExpandNeighbors {
                source_nodes: vec![],
                policy: brain_domain::retrieval::ExpansionPolicy::default(),
            },
        ],
        cost: brain_domain::retrieval::EstimatedCost {
            vector_cost: 10.0,
            keyword_cost: 0.0,
            expansion_cost: 5.0,
            fusion_cost: 1.0,
            ranking_cost: 0.5,
        },
        heuristics_version: 1,
    };

    let spec_executor = RetrievalExecutor::new(&context, ReciprocalRankFusion::default(), NormalizedTieBreakerRanking)
        .with_policy(ExecutionPolicy::Speculative)
        .with_speculation_strategy(Box::new(DummyStrategy { seeds: vec![node_a] }));

    let checker = TriggeredCancellation {
        triggered: Arc::new(AtomicBool::new(true)),
    };
    let mut sink = RecordingSink::new();
    let result = spec_executor.execute_stream(plan, &mut sink, &checker);

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
