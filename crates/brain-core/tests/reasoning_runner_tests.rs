//! Integration tests for ExecutionRunner, DAG scheduling invariants, failure propagation, and cancellation.

use async_trait::async_trait;
use brain_core::reasoning::{
    ExecutionRunner, StepExecutionContext, StepExecutor, StepExecutorRegistry,
};
use brain_domain::{
    DomainError, ExecutionEvent, ExecutionId, ExecutionPlan, PlanStepComplexity, PlanStepId,
    ReasoningPlanStep, ReasoningPlanStepKind, SkippedReason, StepInputs, StepOutput, StepStatus,
    StructuredValue,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct MockStepExecutor {
    counter: Arc<AtomicUsize>,
    should_fail: bool,
}

impl MockStepExecutor {
    fn new(counter: Arc<AtomicUsize>, should_fail: bool) -> Self {
        Self {
            counter,
            should_fail,
        }
    }
}

#[async_trait]
impl StepExecutor for MockStepExecutor {
    async fn execute(
        &self,
        step: &ReasoningPlanStep,
        _ctx: &StepExecutionContext,
        _inputs: &StepInputs,
    ) -> Result<StepOutput, DomainError> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            Err(DomainError::ValidationError {
                message: format!("Mock execution failed for step {}", step.id),
                rule_id: Some("MOCK-FAIL".to_string()),
            })
        } else {
            Ok(StepOutput::new(StructuredValue::String(format!(
                "output_for_step_{}",
                step.id.value()
            ))))
        }
    }
}

#[tokio::test]
async fn test_end_to_end_dag_execution_diamond_pipeline() {
    // Diamond DAG Topology:
    //      Step 1 (Search)
    //       /         \
    //  Step 2 (Memory) Step 3 (Inspect)
    //       \         /
    //      Step 4 (SynthesizeResponse)

    let step1 = ReasoningPlanStep::new(
        PlanStepId::new(1),
        ReasoningPlanStepKind::Search {
            query: "retrieval".to_string(),
        },
        "Root Search Step 1",
        vec![],
        Some(PlanStepComplexity::Low),
    );

    let step2 = ReasoningPlanStep::new(
        PlanStepId::new(2),
        ReasoningPlanStepKind::QueryMemory {
            filter: brain_domain::MemoryFilter::All,
        },
        "Branch Step 2",
        vec![PlanStepId::new(1)],
        Some(PlanStepComplexity::Medium),
    );

    let step3 = ReasoningPlanStep::new(
        PlanStepId::new(3),
        ReasoningPlanStepKind::InspectEntity {
            entity_id: "e1".to_string(),
        },
        "Branch Step 3",
        vec![PlanStepId::new(1)],
        Some(PlanStepComplexity::Medium),
    );

    let step4 = ReasoningPlanStep::new(
        PlanStepId::new(4),
        ReasoningPlanStepKind::SynthesizeResponse,
        "Terminal Synthesis Step 4",
        vec![PlanStepId::new(2), PlanStepId::new(3)],
        Some(PlanStepComplexity::High),
    );

    let plan = ExecutionPlan::new(
        "diamond_plan",
        "test query",
        vec![step1, step2, step3, step4],
    )
    .unwrap();

    let counter = Arc::new(AtomicUsize::new(0));
    let mock_exec = Arc::new(MockStepExecutor::new(counter.clone(), false));

    let mut registry = StepExecutorRegistry::new();
    registry.register(
        &ReasoningPlanStepKind::Search {
            query: "".to_string(),
        },
        mock_exec.clone(),
    );
    registry.register(
        &ReasoningPlanStepKind::QueryMemory {
            filter: brain_domain::MemoryFilter::All,
        },
        mock_exec.clone(),
    );
    registry.register(
        &ReasoningPlanStepKind::InspectEntity {
            entity_id: "".to_string(),
        },
        mock_exec.clone(),
    );
    registry.register(
        &ReasoningPlanStepKind::SynthesizeResponse,
        mock_exec.clone(),
    );

    let runner = ExecutionRunner::new(registry);
    let exec_id = ExecutionId::new();
    let cancel_token = CancellationToken::new();
    let ctx = StepExecutionContext::new(exec_id, cancel_token);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    let state = runner.run_plan(&plan, ctx, event_tx).await.unwrap();

    // Verify invariants:
    // 1. All 4 steps completed
    assert_eq!(state.completed_steps().len(), 4);
    assert_eq!(counter.load(Ordering::SeqCst), 4);

    // 2. Output payloads populated for all completed steps
    for i in 1..=4 {
        let id = PlanStepId::new(i);
        assert_eq!(state.status(id), StepStatus::Completed);
        assert!(state.output(id).is_some());
    }

    // 3. Collect and verify events emitted
    let mut events = Vec::new();
    while let Ok(evt) = event_rx.try_recv() {
        events.push(evt);
    }

    assert!(events
        .iter()
        .any(|e| matches!(e, ExecutionEvent::PlanCompleted { .. })));
}

#[tokio::test]
async fn test_failure_propagation_skips_downstream_dependents() {
    // Pipeline Topology:
    // Step 1 (Search) [Fails] -> Step 2 (SynthesizeResponse)

    let step1 = ReasoningPlanStep::new(
        PlanStepId::new(1),
        ReasoningPlanStepKind::Search {
            query: "fail".to_string(),
        },
        "Failing Step 1",
        vec![],
        Some(PlanStepComplexity::Low),
    );

    let step2 = ReasoningPlanStep::new(
        PlanStepId::new(2),
        ReasoningPlanStepKind::SynthesizeResponse,
        "Dependent Step 2",
        vec![PlanStepId::new(1)],
        Some(PlanStepComplexity::High),
    );

    let plan = ExecutionPlan::new("fail_plan", "query", vec![step1, step2]).unwrap();

    let counter = Arc::new(AtomicUsize::new(0));
    let failing_exec = Arc::new(MockStepExecutor::new(counter.clone(), true));
    let success_exec = Arc::new(MockStepExecutor::new(counter.clone(), false));

    let mut registry = StepExecutorRegistry::new();
    registry.register(
        &ReasoningPlanStepKind::Search {
            query: "".to_string(),
        },
        failing_exec,
    );
    registry.register(&ReasoningPlanStepKind::SynthesizeResponse, success_exec);

    let runner = ExecutionRunner::new(registry);
    let ctx = StepExecutionContext::new(ExecutionId::new(), CancellationToken::new());
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    let state = runner.run_plan(&plan, ctx, event_tx).await.unwrap();

    // Step 1 failed
    assert_eq!(state.status(PlanStepId::new(1)), StepStatus::Failed);
    assert!(state.error(PlanStepId::new(1)).is_some());

    // Step 2 skipped due to upstream failure
    assert_eq!(
        state.status(PlanStepId::new(2)),
        StepStatus::Skipped(SkippedReason::UpstreamFailure)
    );
    assert!(state.output(PlanStepId::new(2)).is_none());

    let mut events = Vec::new();
    while let Ok(evt) = event_rx.try_recv() {
        events.push(evt);
    }

    assert!(events.iter().any(|e| matches!(
        e,
        ExecutionEvent::StepSkipped {
            reason: SkippedReason::UpstreamFailure,
            ..
        }
    )));
    assert!(events
        .iter()
        .any(|e| matches!(e, ExecutionEvent::PlanFailed { .. })));
}

#[tokio::test]
async fn test_cooperative_cancellation_isolation() {
    let step1 = ReasoningPlanStep::new(
        PlanStepId::new(1),
        ReasoningPlanStepKind::SynthesizeResponse,
        "Step 1",
        vec![],
        Some(PlanStepComplexity::Low),
    );

    let plan = ExecutionPlan::new("cancel_plan", "query", vec![step1]).unwrap();

    let counter = Arc::new(AtomicUsize::new(0));
    let mock_exec = Arc::new(MockStepExecutor::new(counter.clone(), false));

    let mut registry = StepExecutorRegistry::new();
    registry.register(&ReasoningPlanStepKind::SynthesizeResponse, mock_exec);

    let runner = ExecutionRunner::new(registry);
    let cancel_token = CancellationToken::new();
    // Cancel immediately before running plan
    cancel_token.cancel();

    let ctx = StepExecutionContext::new(ExecutionId::new(), cancel_token);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    let state = runner.run_plan(&plan, ctx, event_tx).await.unwrap();

    assert_eq!(
        state.status(PlanStepId::new(1)),
        StepStatus::Skipped(SkippedReason::Cancelled)
    );

    let mut events = Vec::new();
    while let Ok(evt) = event_rx.try_recv() {
        events.push(evt);
    }

    assert!(events.iter().any(|e| matches!(
        e,
        ExecutionEvent::StepSkipped {
            reason: SkippedReason::Cancelled,
            ..
        }
    )));
}

#[test]
fn test_artifact_store_provenance_graph_traversal_and_relationships() {
    use brain_domain::{
        ArtifactMetadata, ArtifactStore, EvidenceArtifactKind, ExecutionArtifact,
        ExecutionTimestamp, ProvenanceEdge, ProvenanceRelationship, StructuredValue,
    };

    let mut store = ArtifactStore::new();

    let meta1 = ArtifactMetadata {
        kind: EvidenceArtifactKind::RawData,
        producer_step: PlanStepId::new(1),
        execution_id: ExecutionId::new(),
        created_at: ExecutionTimestamp::now(),
    };
    let art1 = ExecutionArtifact::new(meta1, StructuredValue::String("raw".to_string()));
    let art1_id = store.insert(art1).unwrap();

    let meta2 = ArtifactMetadata {
        kind: EvidenceArtifactKind::Summary,
        producer_step: PlanStepId::new(2),
        execution_id: ExecutionId::new(),
        created_at: ExecutionTimestamp::now(),
    };
    let art2 = ExecutionArtifact::new(meta2, StructuredValue::String("summary".to_string()));
    let art2_id = store.insert(art2).unwrap();

    let meta3 = ArtifactMetadata {
        kind: EvidenceArtifactKind::Claim,
        producer_step: PlanStepId::new(3),
        execution_id: ExecutionId::new(),
        created_at: ExecutionTimestamp::now(),
    };
    let art3 = ExecutionArtifact::new(meta3, StructuredValue::String("claim".to_string()));
    let art3_id = store.insert(art3).unwrap();

    // Provenance chain: art1 -> art2 (DerivedFrom), art2 -> art3 (Summarizes), art1 -> art3 (References)
    let edge1 = ProvenanceEdge::new(
        art1_id,
        art2_id,
        ProvenanceRelationship::DerivedFrom,
        ExecutionTimestamp::now(),
    );
    let edge2 = ProvenanceEdge::new(
        art2_id,
        art3_id,
        ProvenanceRelationship::Summarizes,
        ExecutionTimestamp::now(),
    );
    let edge3 = ProvenanceEdge::new(
        art1_id,
        art3_id,
        ProvenanceRelationship::References,
        ExecutionTimestamp::now(),
    );

    store.add_edge(edge1).unwrap();
    store.add_edge(edge2).unwrap();
    store.add_edge(edge3).unwrap();

    // Verify parents and children queries
    let mut expected_children = vec![art2_id, art3_id];
    expected_children.sort();
    assert_eq!(store.children(art1_id), expected_children);

    let mut expected_parents = vec![art1_id, art2_id];
    expected_parents.sort();
    assert_eq!(store.parents(art3_id), expected_parents);

    // Verify ancestors of art3 reaches art1 and art2 without duplicates
    let ancestors_3 = store.ancestors(art3_id);
    assert_eq!(ancestors_3.len(), 2);
    assert!(ancestors_3.contains(&art1_id));
    assert!(ancestors_3.contains(&art2_id));
}

#[test]
fn test_invalid_provenance_edge_insertion_rejected() {
    use brain_domain::{
        ArtifactMetadata, ArtifactStore, EvidenceArtifactId, EvidenceArtifactKind,
        ExecutionArtifact, ExecutionTimestamp, ProvenanceEdge, ProvenanceRelationship,
        StructuredValue,
    };

    let mut store = ArtifactStore::new();

    let meta1 = ArtifactMetadata {
        kind: EvidenceArtifactKind::RawData,
        producer_step: PlanStepId::new(1),
        execution_id: ExecutionId::new(),
        created_at: ExecutionTimestamp::now(),
    };
    let art1 = ExecutionArtifact::new(meta1, StructuredValue::Null);
    let art1_id = store.insert(art1).unwrap();

    let missing_id = EvidenceArtifactId::new();

    // Edge pointing to missing artifact must be rejected
    let edge = ProvenanceEdge::new(
        art1_id,
        missing_id,
        ProvenanceRelationship::DerivedFrom,
        ExecutionTimestamp::now(),
    );
    let res = store.add_edge(edge);
    assert!(res.is_err());

    // Self-loop edge must be rejected
    let self_loop = ProvenanceEdge::new(
        art1_id,
        art1_id,
        ProvenanceRelationship::DerivedFrom,
        ExecutionTimestamp::now(),
    );
    let self_res = store.add_edge(self_loop);
    assert!(self_res.is_err());
}

#[tokio::test]
async fn test_end_to_end_reasoning_synthesis_pipeline() {
    use brain_core::reasoning::{
        DefaultSynthesisPolicy, EvidenceResolver, EvidenceSelector, SynthesizerService,
    };
    use brain_domain::{EvidenceQuery, SelectionContext, SelectionStrategy};

    let step1 = ReasoningPlanStep::new(
        PlanStepId::new(1),
        ReasoningPlanStepKind::Search {
            query: "synthesis".to_string(),
        },
        "Search Step 1",
        vec![],
        Some(PlanStepComplexity::Low),
    );

    let step2 = ReasoningPlanStep::new(
        PlanStepId::new(2),
        ReasoningPlanStepKind::SynthesizeResponse,
        "Synthesize Step 2",
        vec![PlanStepId::new(1)],
        Some(PlanStepComplexity::High),
    );

    let plan = ExecutionPlan::new("synthesis_pipeline_plan", "query", vec![step1, step2]).unwrap();

    let counter = Arc::new(AtomicUsize::new(0));
    let mock_exec = Arc::new(MockStepExecutor::new(counter.clone(), false));

    let mut registry = StepExecutorRegistry::new();
    registry.register(&ReasoningPlanStepKind::Search { query: "".to_string() }, mock_exec.clone());
    registry.register(&ReasoningPlanStepKind::SynthesizeResponse, mock_exec);

    let runner = ExecutionRunner::new(registry);
    let exec_id = ExecutionId::new();
    let ctx = StepExecutionContext::new(exec_id, CancellationToken::new());
    let (event_tx, _) = mpsc::unbounded_channel();

    let state = runner.run_plan(&plan, ctx, event_tx).await.unwrap();

    let selector = EvidenceSelector::new();
    let resolver = EvidenceResolver::new();
    let policy = DefaultSynthesisPolicy::new();
    let synthesizer = SynthesizerService::new();

    let query = EvidenceQuery::new(SelectionStrategy::All, SelectionContext::new(exec_id));
    let evidence_set = selector.select(&state.artifact_store, &query);
    assert_eq!(evidence_set.len(), 2);

    let views = resolver.resolve(&evidence_set, &state.artifact_store);
    assert_eq!(views.len(), 2);

    let result = synthesizer
        .synthesize(exec_id, &plan, &state, &selector, &policy)
        .unwrap();

    assert_eq!(result.execution_id, exec_id);
    assert_eq!(result.findings.len(), 2);
    assert_eq!(result.evidence_set.len(), 2);
}

#[test]
fn test_nested_algebraic_selection_deduplication() {
    use brain_core::reasoning::EvidenceSelector;
    use brain_domain::{
        ArtifactMetadata, ArtifactStore, EvidenceArtifactKind, EvidenceQuery, ExecutionArtifact,
        ExecutionTimestamp, ProvenanceEdge, ProvenanceRelationship, SelectionContext,
        SelectionStrategy, StructuredValue,
    };

    let mut store = ArtifactStore::new();

    let meta1 = ArtifactMetadata {
        kind: EvidenceArtifactKind::RawData,
        producer_step: PlanStepId::new(1),
        execution_id: ExecutionId::new(),
        created_at: ExecutionTimestamp::now(),
    };
    let art1 = ExecutionArtifact::new(meta1, StructuredValue::String("raw".to_string()));
    let art1_id = store.insert(art1).unwrap();

    let meta2 = ArtifactMetadata {
        kind: EvidenceArtifactKind::Result,
        producer_step: PlanStepId::new(2),
        execution_id: ExecutionId::new(),
        created_at: ExecutionTimestamp::now(),
    };
    let art2 = ExecutionArtifact::new(meta2, StructuredValue::String("res".to_string()));
    let art2_id = store.insert(art2).unwrap();

    let edge = ProvenanceEdge::new(
        art1_id,
        art2_id,
        ProvenanceRelationship::DerivedFrom,
        ExecutionTimestamp::now(),
    );
    store.add_edge(edge).unwrap();

    let selector = EvidenceSelector::new();
    let ctx = SelectionContext::new(ExecutionId::new());

    // Union(ByKind(Result), AncestorsOf(art2_id)) -> art2_id and art1_id
    let strategy = SelectionStrategy::Union(
        Box::new(SelectionStrategy::ByKind(EvidenceArtifactKind::Result)),
        Box::new(SelectionStrategy::AncestorsOf(art2_id)),
    );

    let query = EvidenceQuery::new(strategy, ctx);
    let set = selector.select(&store, &query);

    assert_eq!(set.len(), 2);
    assert!(set.contains(&art1_id));
    assert!(set.contains(&art2_id));
}

#[test]
fn test_empty_evidence_selection_handles_cleanly() {
    use brain_core::reasoning::{
        DefaultSynthesisPolicy, EvidenceSelector, SynthesizerService,
    };
    use brain_domain::ExecutionState;

    let exec_id = brain_domain::ExecutionId::new();
    let step1 = ReasoningPlanStep::new(
        PlanStepId::new(1),
        ReasoningPlanStepKind::SynthesizeResponse,
        "Step 1",
        vec![],
        None,
    );
    let plan = ExecutionPlan::new("empty_store_plan", "query", vec![step1]).unwrap();
    let state = ExecutionState::default();

    let selector = EvidenceSelector::new();
    let policy = DefaultSynthesisPolicy::new();
    let synthesizer = SynthesizerService::new();

    let result = synthesizer
        .synthesize(exec_id, &plan, &state, &selector, &policy)
        .unwrap();

    assert_eq!(result.findings.len(), 0);
    assert!(result.evidence_set.is_empty());
}
