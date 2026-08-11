//! ReasoningRuntime composition façade coordinating end-to-end reasoning cycles.

use crate::reasoning::{
    CandidateExtractorService, ConsolidationService, DefaultConsolidationPolicy,
    DefaultMatchAssessmentPolicy, DefaultReflectionPolicy, DefaultSynthesisPolicy,
    EvidenceSelector, KnowledgeGraphMatcherService, MatchAssessmentService,
    MemoryMutationPlannerService, MemoryStewardshipExecutor, MemoryStewardshipExecutorService,
    ReflectionService, SynthesizerService,
};
use brain_domain::{
    DomainError, ReasoningPhaseReport, ReasoningSession, RuntimeContext, RuntimeExecutionReport,
    RuntimePolicySet, SessionTransition, StewardshipPhaseReport,
};
use std::sync::Arc;

/// Composition façade providing a single entry point for end-to-end reasoning runtime cycles.
///
/// Invariants:
/// - `ReasoningRuntime` may orchestrate capability services but may not implement capability logic (zero business logic façade).
/// - Asynchronous entry point (`async fn run_cycle`).
/// - The runtime façade is the only supported orchestration entry point for end-to-end reasoning execution.
#[derive(Debug, Clone)]
pub struct ReasoningRuntime {
    synthesizer: Arc<SynthesizerService>,
    reflection_service: Arc<ReflectionService>,
    candidate_extractor: Arc<CandidateExtractorService>,
    #[allow(dead_code)]
    matcher: Arc<KnowledgeGraphMatcherService>,
    match_assessment_service: Arc<MatchAssessmentService>,
    consolidation_service: Arc<ConsolidationService>,
    mutation_planner: Arc<MemoryMutationPlannerService>,
    stewardship_executor: Arc<MemoryStewardshipExecutorService>,
}

impl ReasoningRuntime {
    /// Instantiates a new `ReasoningRuntime` façade with default capability services.
    pub fn new() -> Self {
        Self {
            synthesizer: Arc::new(SynthesizerService::new()),
            reflection_service: Arc::new(ReflectionService::new()),
            candidate_extractor: Arc::new(CandidateExtractorService::new()),
            matcher: Arc::new(KnowledgeGraphMatcherService::new()),
            match_assessment_service: Arc::new(MatchAssessmentService::new()),
            consolidation_service: Arc::new(ConsolidationService::new()),
            mutation_planner: Arc::new(MemoryMutationPlannerService::new()),
            stewardship_executor: Arc::new(MemoryStewardshipExecutorService::new()),
        }
    }

    /// Executes an end-to-end reasoning cycle asynchronously with default policy set.
    pub async fn run_cycle(
        &self,
        ctx: &RuntimeContext,
        query: &str,
    ) -> Result<RuntimeExecutionReport, DomainError> {
        self.run_cycle_with_policy(ctx, query, RuntimePolicySet::default())
            .await
    }

    /// Executes an end-to-end reasoning cycle asynchronously with a specific `RuntimePolicySet`.
    pub async fn run_cycle_with_policy(
        &self,
        ctx: &RuntimeContext,
        query: &str,
        policy_set: RuntimePolicySet,
    ) -> Result<RuntimeExecutionReport, DomainError> {
        let exec_id = ctx.execution_id;
        let mut session = ReasoningSession::new(exec_id);
        session = session.transition(SessionTransition::StartExecution)?;

        // 1. Synthesis phase
        let plan = brain_domain::ExecutionPlan::new(
            "runtime_plan",
            query,
            vec![brain_domain::ReasoningPlanStep::new(
                brain_domain::PlanStepId::new(1),
                brain_domain::ReasoningPlanStepKind::SynthesizeResponse,
                "Synthesize",
                vec![],
                None,
            )],
        )?;
        let state = brain_domain::ExecutionState::default();
        let selector = EvidenceSelector::new();
        let synth_policy = DefaultSynthesisPolicy::new();

        let reasoning_result =
            self.synthesizer
                .synthesize(exec_id, &plan, &state, &selector, &synth_policy)?;
        session = session.transition(SessionTransition::AttachReasoningResult(
            reasoning_result.clone(),
        ))?;

        // 2. Reflection phase
        let refl_policy = DefaultReflectionPolicy::new();
        let reflection_report = self
            .reflection_service
            .reflect(&reasoning_result, &refl_policy)?;
        session = session.transition(SessionTransition::AttachReflectionReport(
            reflection_report.clone(),
        ))?;

        // 3. Knowledge Candidate Extraction & Graph Matching phase
        let candidates = self
            .candidate_extractor
            .extract_candidates(&reasoning_result, &reflection_report);

        // 4. Consolidation & Match Assessment phase
        let match_assess_policy = DefaultMatchAssessmentPolicy::new();
        let consolidation_policy = DefaultConsolidationPolicy::new();
        let consolidation_report = self.consolidation_service.consolidate(
            exec_id,
            &candidates,
            &consolidation_policy,
            &self.match_assessment_service,
            &match_assess_policy,
        )?;

        // 5. Evolution planning phase
        let evo_planner = crate::reasoning::EvolutionPlannerService::new();
        let evolution_plan = evo_planner.plan_evolution(&reflection_report)?;
        session = session.transition(SessionTransition::AttachEvolutionPlan(evolution_plan))?;

        // 6. Memory Stewardship Execution phase
        let mutation_batch = self
            .mutation_planner
            .plan_mutations(&consolidation_report)?;
        let (summary, audit_log) = self.stewardship_executor.execute_batch(&mutation_batch)?;

        session = session.transition(SessionTransition::Complete)?;

        let provenance = brain_domain::RuntimeProvenance::new(exec_id, policy_set.clone(), None);

        let reasoning_phase = ReasoningPhaseReport::new(
            Some(reasoning_result),
            Some(reflection_report),
            Some(consolidation_report),
        );

        let stewardship_phase = StewardshipPhaseReport::new(Some(summary), Some(audit_log));

        Ok(RuntimeExecutionReport::new(
            exec_id,
            policy_set,
            provenance,
            session,
            reasoning_phase,
            stewardship_phase,
        ))
    }
}

impl Default for ReasoningRuntime {
    fn default() -> Self {
        Self::new()
    }
}
