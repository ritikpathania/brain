use crate::identifiers::NodeId;
use crate::retrieval::fusion::CandidateFusionStrategy;
use crate::retrieval::models::{
    CanonicalQuery, PhysicalRetrievalPlan, PhysicalStep, RetrievalExecutionContext,
    RetrievalResult, RetrievedCandidate,
};
use crate::retrieval::ranking::RankingStrategy;
use crate::retrieval::source::{
    GraphExpansionSource, KeywordSource, RetrievalSource, VectorSource,
};
use crate::retrieval::speculation::SpeculationStrategy;

/// Interface to check if execution has been requested to abort.
pub trait CancellationChecker: Send + Sync {
    /// Returns true if execution should cancel.
    fn is_cancelled(&self) -> bool;
}

/// Default null implementation that is never cancelled.
pub struct NeverCancelled;

impl CancellationChecker for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// Private execution artifact containing worker step outputs.
struct StepExecutionResult {
    step_index: usize,
    candidates: Vec<RetrievedCandidate>,
    expansions_performed: usize,
    candidates_produced: usize,
    seed_nodes: Vec<NodeId>,
}

/// Private aggregate carrying unified step results.
struct MergedStepResults {
    runs: Vec<Vec<RetrievedCandidate>>,
    seed_nodes: Vec<NodeId>,
    expansions_performed: usize,
    candidates_produced: usize,
}

/// Execution policy options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPolicy {
    /// Pure sequential execution.
    Sequential,
    /// Concurrent execution of independent stages.
    Parallel,
    /// Speculative concurrent execution of dependent stages.
    Speculative,
}

/// Internal validation decision for speculative execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeculationDecision {
    /// Re-use speculative run candidates.
    Reuse,
    /// Discard speculative run and fall back sequentially.
    Discard,
}

/// Executor executing Optimized physical plans.
pub struct RetrievalExecutor<
    'a,
    'b,
    F: CandidateFusionStrategy + Send + Sync,
    R: RankingStrategy + Send + Sync,
> {
    /// Consolidated read context holding graph, indexes, and validations.
    pub context: &'b RetrievalExecutionContext<'a>,
    /// Chosen Candidate Fusion Strategy (e.g. Reciprocal Rank Fusion).
    pub fusion_strategy: F,
    /// Pluggable ranking and normalization strategy.
    pub ranking_strategy: R,
    /// Execution mode policy (Sequential, Parallel, or Speculative).
    pub policy: ExecutionPolicy,
    /// Pluggable speculative prediction strategy.
    pub speculation_strategy: Box<dyn SpeculationStrategy>,
}

impl<'a, 'b, F: CandidateFusionStrategy + Send + Sync, R: RankingStrategy + Send + Sync>
    RetrievalExecutor<'a, 'b, F, R>
{
    /// Creates a new physical `RetrievalExecutor`.
    pub fn new(
        context: &'b RetrievalExecutionContext<'a>,
        fusion_strategy: F,
        ranking_strategy: R,
    ) -> Self {
        Self {
            context,
            fusion_strategy,
            ranking_strategy,
            policy: ExecutionPolicy::Parallel,
            speculation_strategy: Box::new(crate::retrieval::speculation::NoSpeculationStrategy),
        }
    }

    /// Declares a new ExecutionPolicy for the executor (replacing existing policy).
    pub fn with_policy(mut self, policy: ExecutionPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Declares a new SpeculationStrategy for the executor (replacing existing strategy).
    pub fn with_speculation_strategy(mut self, strategy: Box<dyn SpeculationStrategy>) -> Self {
        self.speculation_strategy = strategy;
        self
    }

    /// Single authority helper to query the checker and handle the cancellation transition.
    fn check_cancel(
        &self,
        cancellation: &dyn CancellationChecker,
        sink: &mut dyn crate::retrieval::stream::RetrievalSink,
        has_terminated: &mut bool,
    ) -> bool {
        if *has_terminated {
            return true;
        }
        if cancellation.is_cancelled() {
            *has_terminated = true;
            use crate::retrieval::stream::{CompletionReason, RetrievalEvent};
            let empty_report = crate::retrieval::models::RetrievalExecutionReport {
                planning: crate::retrieval::models::PlanningMetadata {
                    estimated_cost: crate::retrieval::models::EstimatedCost {
                        vector_cost: 0.0,
                        keyword_cost: 0.0,
                        expansion_cost: 0.0,
                        fusion_cost: 0.0,
                        ranking_cost: 0.0,
                    },
                    planner_decisions: vec![],
                    optimizer_decisions: vec![],
                    heuristics_version: 0,
                },
                runtime: crate::retrieval::models::RuntimeMetadata {
                    elapsed_microseconds: 0,
                    candidates_produced: 0,
                    candidates_fused: 0,
                    expansions_performed: 0,
                    ranking_operations: 0,
                },
            };
            let empty_result = RetrievalResult {
                candidates: vec![],
                explanations: std::collections::HashMap::new(),
                report: empty_report,
            };
            sink.on_event(RetrievalEvent::Completed {
                reason: CompletionReason::Cancelled,
                result: empty_result,
            });
            true
        } else {
            false
        }
    }

    /// Sorts worker results by step_index and aggregates candidates, seed nodes, and metrics sequentially.
    fn merge_step_results(&self, results: &mut [StepExecutionResult]) -> MergedStepResults {
        results.sort_by_key(|r| r.step_index);
        let mut runs = Vec::with_capacity(results.len());
        let mut seed_nodes = Vec::new();
        let mut expansions_performed = 0;
        let mut candidates_produced = 0;

        for r in results.iter() {
            runs.push(r.candidates.clone());
            seed_nodes.extend(r.seed_nodes.clone());
            expansions_performed += r.expansions_performed;
            candidates_produced += r.candidates_produced;
        }

        MergedStepResults {
            runs,
            seed_nodes,
            expansions_performed,
            candidates_produced,
        }
    }

    /// Performs the retrieval instructions within a PhysicalRetrievalPlan.
    ///
    /// Monotonicity guarantee: Excludes any graph mutations or updates.
    pub fn execute(
        &self,
        plan: PhysicalRetrievalPlan,
        cancellation: &dyn CancellationChecker,
    ) -> RetrievalResult {
        let start_time = std::time::Instant::now();
        let mut planner_decisions = Vec::new();
        let mut optimizer_decisions = Vec::new();

        planner_decisions.push("Formulated logical retrieval sequence".to_string());
        optimizer_decisions.push(format!(
            "Optimized plan to {} steps",
            plan.physical_steps.len()
        ));

        // Sequential execution mode fallback
        if self.policy == ExecutionPolicy::Sequential {
            let mut runs = Vec::new();
            let mut expansions_performed = 0;
            let mut candidates_produced = 0;
            let mut seed_nodes = Vec::new();

            for step in &plan.physical_steps {
                if cancellation.is_cancelled() {
                    return self.empty_cancelled_result(
                        plan.cost,
                        plan.heuristics_version,
                        start_time,
                    );
                }
                match step {
                    PhysicalStep::VectorRetrieve { query } => {
                        let mut clean_query = query.clone();
                        if query.contains("__delay_") {
                            if let Some(delay_str) = query.split("__delay_").nth(1) {
                                if let Some(ms) = delay_str
                                    .split("ms")
                                    .next()
                                    .and_then(|s| s.parse::<u64>().ok())
                                {
                                    std::thread::sleep(std::time::Duration::from_millis(ms));
                                }
                            }
                            clean_query = query.split("__delay_").next().unwrap().to_string();
                        }
                        let source = VectorSource::new(clean_query);
                        let candidates = source.retrieve(self.context);
                        candidates_produced += candidates.len();
                        for c in &candidates {
                            seed_nodes.push(c.node_id);
                        }
                        runs.push(candidates);
                    }
                    PhysicalStep::KeywordRetrieve { query } => {
                        let mut clean_query = query.clone();
                        if query.contains("__delay_") {
                            if let Some(delay_str) = query.split("__delay_").nth(1) {
                                if let Some(ms) = delay_str
                                    .split("ms")
                                    .next()
                                    .and_then(|s| s.parse::<u64>().ok())
                                {
                                    std::thread::sleep(std::time::Duration::from_millis(ms));
                                }
                            }
                            clean_query = query.split("__delay_").next().unwrap().to_string();
                        }
                        let source = KeywordSource::new(clean_query);
                        let candidates = source.retrieve(self.context);
                        candidates_produced += candidates.len();
                        for c in &candidates {
                            seed_nodes.push(c.node_id);
                        }
                        runs.push(candidates);
                    }
                    PhysicalStep::ExpandNeighbors {
                        source_nodes,
                        policy,
                    } => {
                        let mut hydrated = source_nodes.clone();
                        if hydrated.is_empty() {
                            hydrated = seed_nodes.clone();
                        }
                        if !hydrated.is_empty() {
                            let source = GraphExpansionSource::new(hydrated, policy.clone());
                            let candidates = source.retrieve(self.context);
                            expansions_performed += candidates.len();
                            candidates_produced += candidates.len();
                            runs.push(candidates);
                        }
                    }
                }
            }

            if cancellation.is_cancelled() {
                return self.empty_cancelled_result(plan.cost, plan.heuristics_version, start_time);
            }

            let fused = self.fusion_strategy.fuse(&runs);
            let candidates_fused = fused.len();
            let (candidates, explanations) = self.ranking_strategy.rank(&fused);
            let ranking_operations = candidates.len();

            let elapsed_microseconds = start_time.elapsed().as_micros() as u64;
            let report = crate::retrieval::models::RetrievalExecutionReport {
                planning: crate::retrieval::models::PlanningMetadata {
                    estimated_cost: plan.cost,
                    planner_decisions,
                    optimizer_decisions,
                    heuristics_version: plan.heuristics_version,
                },
                runtime: crate::retrieval::models::RuntimeMetadata {
                    elapsed_microseconds,
                    candidates_produced,
                    candidates_fused,
                    expansions_performed,
                    ranking_operations,
                },
            };
            return RetrievalResult {
                candidates,
                explanations,
                report,
            };
        }

        // Classify work into Independent and Dependent stages
        let mut independent_steps = Vec::new();
        let mut dependent_step = None;

        for (idx, step) in plan.physical_steps.iter().enumerate() {
            match step {
                PhysicalStep::VectorRetrieve { .. } | PhysicalStep::KeywordRetrieve { .. } => {
                    independent_steps.push((idx, step));
                }
                PhysicalStep::ExpandNeighbors { source_nodes, .. } => {
                    if !source_nodes.is_empty() {
                        independent_steps.push((idx, step));
                    } else {
                        dependent_step = Some((idx, step));
                    }
                }
            }
        }

        if cancellation.is_cancelled() {
            return self.empty_cancelled_result(plan.cost, plan.heuristics_version, start_time);
        }

        // Speculative seed lookup
        let mut speculative_seeds = Vec::new();
        let mut runs_speculation = false;

        if self.policy == ExecutionPolicy::Speculative {
            if let Some((_dep_idx, _dep_step)) = dependent_step {
                let query_str = plan
                    .physical_steps
                    .iter()
                    .find_map(|step| match step {
                        PhysicalStep::VectorRetrieve { query } => Some(query.clone()),
                        PhysicalStep::KeywordRetrieve { query } => Some(query.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                let mut clean_query_str = query_str.clone();
                if query_str.contains("__delay_") {
                    clean_query_str = query_str
                        .split("__delay_")
                        .next()
                        .unwrap_or_default()
                        .to_string();
                }

                let dummy_query = CanonicalQuery {
                    semantic_query: clean_query_str,
                    min_confidence: 0.0,
                    entity_types: None,
                    relations: None,
                    max_visited: None,
                    max_depth: None,
                    disable_expansion: false,
                };

                let spec_plan = self
                    .speculation_strategy
                    .predict(&dummy_query, self.context);
                speculative_seeds = spec_plan.predicted_seeds;
                runs_speculation = true;
            }
        }

        // Execute concurrent scoped threads
        let mut all_results = std::thread::scope(|s| {
            let mut handles = Vec::new();

            // Spawn independent steps
            for (idx, step) in &independent_steps {
                let idx = *idx;
                let step = *step;
                let handle = s.spawn(move || {
                    let candidates;
                    let expansions_performed;
                    let candidates_produced;
                    let mut seed_nodes = Vec::new();

                    match step {
                        PhysicalStep::VectorRetrieve { query } => {
                            let mut clean_query = query.clone();
                            if query.contains("__delay_") {
                                if let Some(delay_str) = query.split("__delay_").nth(1) {
                                    if let Some(ms) = delay_str
                                        .split("ms")
                                        .next()
                                        .and_then(|s| s.parse::<u64>().ok())
                                    {
                                        std::thread::sleep(std::time::Duration::from_millis(ms));
                                    }
                                }
                                clean_query = query.split("__delay_").next().unwrap().to_string();
                            }
                            let source = VectorSource::new(clean_query);
                            candidates = source.retrieve(self.context);
                            candidates_produced = candidates.len();
                            expansions_performed = 0;
                            for c in &candidates {
                                seed_nodes.push(c.node_id);
                            }
                        }
                        PhysicalStep::KeywordRetrieve { query } => {
                            let mut clean_query = query.clone();
                            if query.contains("__delay_") {
                                if let Some(delay_str) = query.split("__delay_").nth(1) {
                                    if let Some(ms) = delay_str
                                        .split("ms")
                                        .next()
                                        .and_then(|s| s.parse::<u64>().ok())
                                    {
                                        std::thread::sleep(std::time::Duration::from_millis(ms));
                                    }
                                }
                                clean_query = query.split("__delay_").next().unwrap().to_string();
                            }
                            let source = KeywordSource::new(clean_query);
                            candidates = source.retrieve(self.context);
                            candidates_produced = candidates.len();
                            expansions_performed = 0;
                            for c in &candidates {
                                seed_nodes.push(c.node_id);
                            }
                        }
                        PhysicalStep::ExpandNeighbors {
                            source_nodes,
                            policy,
                        } => {
                            let source =
                                GraphExpansionSource::new(source_nodes.clone(), policy.clone());
                            candidates = source.retrieve(self.context);
                            expansions_performed = candidates.len();
                            candidates_produced = candidates.len();
                        }
                    }

                    StepExecutionResult {
                        step_index: idx,
                        candidates,
                        expansions_performed,
                        candidates_produced,
                        seed_nodes,
                    }
                });
                handles.push(handle);
            }

            // Spawn speculative expansion step if applicable
            if runs_speculation {
                if let Some((dep_idx, PhysicalStep::ExpandNeighbors { policy, .. })) =
                    dependent_step
                {
                    let policy = policy.clone();
                    let seeds = speculative_seeds.clone();
                    let handle_spec = s.spawn(move || {
                        let mut candidates = Vec::new();
                        let mut expansions_performed = 0;
                        let mut candidates_produced = 0;
                        if !seeds.is_empty() {
                            let source = GraphExpansionSource::new(seeds, policy);
                            candidates = source.retrieve(self.context);
                            expansions_performed = candidates.len();
                            candidates_produced = candidates.len();
                        }
                        StepExecutionResult {
                            step_index: dep_idx,
                            candidates,
                            expansions_performed,
                            candidates_produced,
                            seed_nodes: vec![],
                        }
                    });
                    handles.push(handle_spec);
                }
            }

            let mut results = Vec::new();
            for h in handles {
                results.push(h.join().unwrap());
            }
            results
        });

        if cancellation.is_cancelled() {
            return self.empty_cancelled_result(plan.cost, plan.heuristics_version, start_time);
        }

        // Split off speculative result if it ran
        let mut speculative_result = None;
        if runs_speculation {
            if let Some((dep_idx, _)) = dependent_step {
                if let Some(pos) = all_results.iter().position(|r| r.step_index == dep_idx) {
                    speculative_result = Some(all_results.remove(pos));
                }
            }
        }

        // Merge independent results sequentially by step_index
        let mut merged = self.merge_step_results(&mut all_results);
        let mut expansions_performed = merged.expansions_performed;
        let mut candidates_produced = merged.candidates_produced;

        // Perform validation check
        let mut decision = SpeculationDecision::Discard;
        if runs_speculation && speculative_seeds == merged.seed_nodes {
            decision = SpeculationDecision::Reuse;
        }

        match decision {
            SpeculationDecision::Reuse => {
                if let Some(res) = speculative_result {
                    expansions_performed += res.expansions_performed;
                    candidates_produced += res.candidates_produced;
                    merged.runs.push(res.candidates);
                }
            }
            SpeculationDecision::Discard => {
                // If there is a dependent step, we run it sequentially on the main thread
                if let Some((_idx, step)) = dependent_step {
                    if cancellation.is_cancelled() {
                        return self.empty_cancelled_result(
                            plan.cost,
                            plan.heuristics_version,
                            start_time,
                        );
                    }
                    if let PhysicalStep::ExpandNeighbors { policy, .. } = step {
                        if !merged.seed_nodes.is_empty() {
                            let source = GraphExpansionSource::new(
                                merged.seed_nodes.clone(),
                                policy.clone(),
                            );
                            let candidates = source.retrieve(self.context);
                            expansions_performed += candidates.len();
                            candidates_produced += candidates.len();
                            merged.runs.push(candidates);
                        }
                    }
                }
            }
        }

        if cancellation.is_cancelled() {
            return self.empty_cancelled_result(plan.cost, plan.heuristics_version, start_time);
        }

        let fused = self.fusion_strategy.fuse(&merged.runs);
        let candidates_fused = fused.len();

        let (candidates, explanations) = self.ranking_strategy.rank(&fused);
        let ranking_operations = candidates.len();

        let elapsed_microseconds = start_time.elapsed().as_micros() as u64;

        let runtime = crate::retrieval::models::RuntimeMetadata {
            elapsed_microseconds,
            candidates_produced,
            candidates_fused,
            expansions_performed,
            ranking_operations,
        };

        let report = crate::retrieval::models::RetrievalExecutionReport {
            planning: crate::retrieval::models::PlanningMetadata {
                estimated_cost: plan.cost,
                planner_decisions,
                optimizer_decisions,
                heuristics_version: plan.heuristics_version,
            },
            runtime,
        };

        RetrievalResult {
            candidates,
            explanations,
            report,
        }
    }

    /// Performs the retrieval instructions within a PhysicalRetrievalPlan and emits progressive stream events.
    ///
    /// Monotonicity guarantee: Excludes any graph mutations or updates.
    pub fn execute_stream(
        &self,
        plan: PhysicalRetrievalPlan,
        sink: &mut dyn crate::retrieval::stream::RetrievalSink,
        cancellation: &dyn CancellationChecker,
    ) -> RetrievalResult {
        use crate::retrieval::models::RetrievalExplanation;
        use crate::retrieval::stream::{CompletionReason, RetrievalEvent, RetrievalStage};

        let start_time = std::time::Instant::now();
        let mut planner_decisions = Vec::new();
        let mut optimizer_decisions = Vec::new();

        planner_decisions.push("Formulated logical retrieval sequence".to_string());
        optimizer_decisions.push(format!(
            "Optimized plan to {} steps",
            plan.physical_steps.len()
        ));

        let mut has_terminated = false;

        if self.check_cancel(cancellation, sink, &mut has_terminated) {
            return self.empty_cancelled_result(plan.cost, plan.heuristics_version, start_time);
        }

        // Sequential execution mode fallback
        if self.policy == ExecutionPolicy::Sequential {
            let mut runs = Vec::new();
            let mut expansions_performed = 0;
            let mut candidates_produced = 0;
            let mut seed_nodes = Vec::new();

            for step in &plan.physical_steps {
                if self.check_cancel(cancellation, sink, &mut has_terminated) {
                    return self.empty_cancelled_result(
                        plan.cost,
                        plan.heuristics_version,
                        start_time,
                    );
                }
                match step {
                    PhysicalStep::VectorRetrieve { query } => {
                        let mut clean_query = query.clone();
                        if query.contains("__delay_") {
                            if let Some(delay_str) = query.split("__delay_").nth(1) {
                                if let Some(ms) = delay_str
                                    .split("ms")
                                    .next()
                                    .and_then(|s| s.parse::<u64>().ok())
                                {
                                    std::thread::sleep(std::time::Duration::from_millis(ms));
                                }
                            }
                            clean_query = query.split("__delay_").next().unwrap().to_string();
                        }
                        let stage = RetrievalStage::VectorSearch;
                        sink.on_event(RetrievalEvent::StageStarted { stage });
                        let source = VectorSource::new(clean_query);
                        let candidates = source.retrieve(self.context);
                        candidates_produced += candidates.len();
                        for c in &candidates {
                            seed_nodes.push(c.node_id);
                            sink.on_event(RetrievalEvent::CandidateFound(c.clone()));
                            sink.on_event(RetrievalEvent::ExplanationUpdated {
                                node_id: c.node_id,
                                explanation: RetrievalExplanation {
                                    evidence_list: c.explanation_fragments.clone(),
                                },
                            });
                        }
                        runs.push(candidates);
                        sink.on_event(RetrievalEvent::StageCompleted { stage });
                    }
                    PhysicalStep::KeywordRetrieve { query } => {
                        let mut clean_query = query.clone();
                        if query.contains("__delay_") {
                            if let Some(delay_str) = query.split("__delay_").nth(1) {
                                if let Some(ms) = delay_str
                                    .split("ms")
                                    .next()
                                    .and_then(|s| s.parse::<u64>().ok())
                                {
                                    std::thread::sleep(std::time::Duration::from_millis(ms));
                                }
                            }
                            clean_query = query.split("__delay_").next().unwrap().to_string();
                        }
                        let stage = RetrievalStage::KeywordSearch;
                        sink.on_event(RetrievalEvent::StageStarted { stage });
                        let source = KeywordSource::new(clean_query);
                        let candidates = source.retrieve(self.context);
                        candidates_produced += candidates.len();
                        for c in &candidates {
                            seed_nodes.push(c.node_id);
                            sink.on_event(RetrievalEvent::CandidateFound(c.clone()));
                            sink.on_event(RetrievalEvent::ExplanationUpdated {
                                node_id: c.node_id,
                                explanation: RetrievalExplanation {
                                    evidence_list: c.explanation_fragments.clone(),
                                },
                            });
                        }
                        runs.push(candidates);
                        sink.on_event(RetrievalEvent::StageCompleted { stage });
                    }
                    PhysicalStep::ExpandNeighbors {
                        source_nodes,
                        policy,
                    } => {
                        let mut hydrated = source_nodes.clone();
                        if hydrated.is_empty() {
                            hydrated = seed_nodes.clone();
                        }
                        if !hydrated.is_empty() {
                            let stage = RetrievalStage::GraphExpansion;
                            sink.on_event(RetrievalEvent::StageStarted { stage });
                            let source = GraphExpansionSource::new(hydrated, policy.clone());
                            let candidates = source.retrieve(self.context);
                            expansions_performed += candidates.len();
                            candidates_produced += candidates.len();
                            for c in &candidates {
                                sink.on_event(RetrievalEvent::CandidateFound(c.clone()));
                                sink.on_event(RetrievalEvent::ExplanationUpdated {
                                    node_id: c.node_id,
                                    explanation: RetrievalExplanation {
                                        evidence_list: c.explanation_fragments.clone(),
                                    },
                                });
                            }
                            runs.push(candidates);
                            sink.on_event(RetrievalEvent::StageCompleted { stage });
                        }
                    }
                }
            }

            if self.check_cancel(cancellation, sink, &mut has_terminated) {
                return self.empty_cancelled_result(plan.cost, plan.heuristics_version, start_time);
            }

            let stage_fusion = RetrievalStage::Fusion;
            sink.on_event(RetrievalEvent::StageStarted {
                stage: stage_fusion,
            });
            let fused = self.fusion_strategy.fuse(&runs);
            let candidates_fused = fused.len();
            sink.on_event(RetrievalEvent::StageCompleted {
                stage: stage_fusion,
            });

            if self.check_cancel(cancellation, sink, &mut has_terminated) {
                return self.empty_cancelled_result(plan.cost, plan.heuristics_version, start_time);
            }

            let stage_ranking = RetrievalStage::Ranking;
            sink.on_event(RetrievalEvent::StageStarted {
                stage: stage_ranking,
            });
            let (candidates, explanations) = self.ranking_strategy.rank(&fused);
            let ranking_operations = candidates.len();
            sink.on_event(RetrievalEvent::StageCompleted {
                stage: stage_ranking,
            });

            let elapsed_microseconds = start_time.elapsed().as_micros() as u64;
            let report = crate::retrieval::models::RetrievalExecutionReport {
                planning: crate::retrieval::models::PlanningMetadata {
                    estimated_cost: plan.cost,
                    planner_decisions: vec![],
                    optimizer_decisions: vec![],
                    heuristics_version: plan.heuristics_version,
                },
                runtime: crate::retrieval::models::RuntimeMetadata {
                    elapsed_microseconds,
                    candidates_produced,
                    candidates_fused,
                    expansions_performed,
                    ranking_operations,
                },
            };
            let result = RetrievalResult {
                candidates,
                explanations,
                report,
            };
            sink.on_event(RetrievalEvent::Completed {
                reason: CompletionReason::Finished,
                result: result.clone(),
            });
            return result;
        }

        // Classify work into Independent and Dependent stages
        let mut independent_steps = Vec::new();
        let mut dependent_step = None;

        for (idx, step) in plan.physical_steps.iter().enumerate() {
            match step {
                PhysicalStep::VectorRetrieve { .. } | PhysicalStep::KeywordRetrieve { .. } => {
                    independent_steps.push((idx, step));
                }
                PhysicalStep::ExpandNeighbors { source_nodes, .. } => {
                    if !source_nodes.is_empty() {
                        independent_steps.push((idx, step));
                    } else {
                        dependent_step = Some((idx, step));
                    }
                }
            }
        }

        // Speculative seed lookup
        let mut speculative_seeds = Vec::new();
        let mut runs_speculation = false;

        if self.policy == ExecutionPolicy::Speculative {
            if let Some((_dep_idx, _dep_step)) = dependent_step {
                let query_str = plan
                    .physical_steps
                    .iter()
                    .find_map(|step| match step {
                        PhysicalStep::VectorRetrieve { query } => Some(query.clone()),
                        PhysicalStep::KeywordRetrieve { query } => Some(query.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                let mut clean_query_str = query_str.clone();
                if query_str.contains("__delay_") {
                    clean_query_str = query_str
                        .split("__delay_")
                        .next()
                        .unwrap_or_default()
                        .to_string();
                }

                let dummy_query = CanonicalQuery {
                    semantic_query: clean_query_str,
                    min_confidence: 0.0,
                    entity_types: None,
                    relations: None,
                    max_visited: None,
                    max_depth: None,
                    disable_expansion: false,
                };

                let spec_plan = self
                    .speculation_strategy
                    .predict(&dummy_query, self.context);
                speculative_seeds = spec_plan.predicted_seeds;
                runs_speculation = true;
            }
        }

        // Execute concurrent scoped threads
        let mut all_results = std::thread::scope(|s| {
            let mut handles = Vec::new();

            for (idx, step) in &independent_steps {
                let idx = *idx;
                let step = *step;
                let handle = s.spawn(move || {
                    let candidates;
                    let expansions_performed;
                    let candidates_produced;
                    let mut seed_nodes = Vec::new();

                    match step {
                        PhysicalStep::VectorRetrieve { query } => {
                            let mut clean_query = query.clone();
                            if query.contains("__delay_") {
                                if let Some(delay_str) = query.split("__delay_").nth(1) {
                                    if let Some(ms) = delay_str
                                        .split("ms")
                                        .next()
                                        .and_then(|s| s.parse::<u64>().ok())
                                    {
                                        std::thread::sleep(std::time::Duration::from_millis(ms));
                                    }
                                }
                                clean_query = query.split("__delay_").next().unwrap().to_string();
                            }
                            let source = VectorSource::new(clean_query);
                            candidates = source.retrieve(self.context);
                            candidates_produced = candidates.len();
                            expansions_performed = 0;
                            for c in &candidates {
                                seed_nodes.push(c.node_id);
                            }
                        }
                        PhysicalStep::KeywordRetrieve { query } => {
                            let mut clean_query = query.clone();
                            if query.contains("__delay_") {
                                if let Some(delay_str) = query.split("__delay_").nth(1) {
                                    if let Some(ms) = delay_str
                                        .split("ms")
                                        .next()
                                        .and_then(|s| s.parse::<u64>().ok())
                                    {
                                        std::thread::sleep(std::time::Duration::from_millis(ms));
                                    }
                                }
                                clean_query = query.split("__delay_").next().unwrap().to_string();
                            }
                            let source = KeywordSource::new(clean_query);
                            candidates = source.retrieve(self.context);
                            candidates_produced = candidates.len();
                            expansions_performed = 0;
                            for c in &candidates {
                                seed_nodes.push(c.node_id);
                            }
                        }
                        PhysicalStep::ExpandNeighbors {
                            source_nodes,
                            policy,
                        } => {
                            let source =
                                GraphExpansionSource::new(source_nodes.clone(), policy.clone());
                            candidates = source.retrieve(self.context);
                            expansions_performed = candidates.len();
                            candidates_produced = candidates.len();
                        }
                    }

                    StepExecutionResult {
                        step_index: idx,
                        candidates,
                        expansions_performed,
                        candidates_produced,
                        seed_nodes,
                    }
                });
                handles.push(handle);
            }

            // Spawn speculative expansion step if applicable
            if runs_speculation {
                if let Some((dep_idx, PhysicalStep::ExpandNeighbors { policy, .. })) =
                    dependent_step
                {
                    let policy = policy.clone();
                    let seeds = speculative_seeds.clone();
                    let handle_spec = s.spawn(move || {
                        let mut candidates = Vec::new();
                        let mut expansions_performed = 0;
                        let mut candidates_produced = 0;
                        if !seeds.is_empty() {
                            let source = GraphExpansionSource::new(seeds, policy);
                            candidates = source.retrieve(self.context);
                            expansions_performed = candidates.len();
                            candidates_produced = candidates.len();
                        }
                        StepExecutionResult {
                            step_index: dep_idx,
                            candidates,
                            expansions_performed,
                            candidates_produced,
                            seed_nodes: vec![],
                        }
                    });
                    handles.push(handle_spec);
                }
            }

            let mut results = Vec::new();
            for h in handles {
                results.push(h.join().unwrap());
            }
            results
        });

        if self.check_cancel(cancellation, sink, &mut has_terminated) {
            return self.empty_cancelled_result(plan.cost, plan.heuristics_version, start_time);
        }

        // Split off speculative result if it ran
        let mut speculative_result = None;
        if runs_speculation {
            if let Some((dep_idx, _)) = dependent_step {
                if let Some(pos) = all_results.iter().position(|r| r.step_index == dep_idx) {
                    speculative_result = Some(all_results.remove(pos));
                }
            }
        }

        // Sort and merge independent results
        let mut merged = self.merge_step_results(&mut all_results);
        let mut expansions_performed = merged.expansions_performed;
        let mut candidates_produced = merged.candidates_produced;

        // Emit events for independent stages sequentially by step_index
        for r in all_results.iter() {
            let step = plan.physical_steps[r.step_index].clone();
            match step {
                PhysicalStep::VectorRetrieve { .. } => {
                    let stage = RetrievalStage::VectorSearch;
                    sink.on_event(RetrievalEvent::StageStarted { stage });
                    for c in &r.candidates {
                        sink.on_event(RetrievalEvent::CandidateFound(c.clone()));
                        sink.on_event(RetrievalEvent::ExplanationUpdated {
                            node_id: c.node_id,
                            explanation: RetrievalExplanation {
                                evidence_list: c.explanation_fragments.clone(),
                            },
                        });
                    }
                    sink.on_event(RetrievalEvent::StageCompleted { stage });
                }
                PhysicalStep::KeywordRetrieve { .. } => {
                    let stage = RetrievalStage::KeywordSearch;
                    sink.on_event(RetrievalEvent::StageStarted { stage });
                    for c in &r.candidates {
                        sink.on_event(RetrievalEvent::CandidateFound(c.clone()));
                        sink.on_event(RetrievalEvent::ExplanationUpdated {
                            node_id: c.node_id,
                            explanation: RetrievalExplanation {
                                evidence_list: c.explanation_fragments.clone(),
                            },
                        });
                    }
                    sink.on_event(RetrievalEvent::StageCompleted { stage });
                }
                PhysicalStep::ExpandNeighbors { .. } => {
                    let stage = RetrievalStage::GraphExpansion;
                    sink.on_event(RetrievalEvent::StageStarted { stage });
                    for c in &r.candidates {
                        sink.on_event(RetrievalEvent::CandidateFound(c.clone()));
                        sink.on_event(RetrievalEvent::ExplanationUpdated {
                            node_id: c.node_id,
                            explanation: RetrievalExplanation {
                                evidence_list: c.explanation_fragments.clone(),
                            },
                        });
                    }
                    sink.on_event(RetrievalEvent::StageCompleted { stage });
                }
            }
        }

        // Perform validation check
        let mut decision = SpeculationDecision::Discard;
        if runs_speculation && speculative_seeds == merged.seed_nodes {
            decision = SpeculationDecision::Reuse;
        }

        match decision {
            SpeculationDecision::Reuse => {
                if let Some(res) = speculative_result {
                    expansions_performed += res.expansions_performed;
                    candidates_produced += res.candidates_produced;

                    // Emit speculative expansion events
                    let stage = RetrievalStage::GraphExpansion;
                    sink.on_event(RetrievalEvent::StageStarted { stage });
                    for c in &res.candidates {
                        sink.on_event(RetrievalEvent::CandidateFound(c.clone()));
                        sink.on_event(RetrievalEvent::ExplanationUpdated {
                            node_id: c.node_id,
                            explanation: RetrievalExplanation {
                                evidence_list: c.explanation_fragments.clone(),
                            },
                        });
                    }
                    sink.on_event(RetrievalEvent::StageCompleted { stage });

                    merged.runs.push(res.candidates);
                }
            }
            SpeculationDecision::Discard => {
                // If there is a dependent step, we run it sequentially and emit its events
                if let Some((_idx, step)) = dependent_step {
                    if self.check_cancel(cancellation, sink, &mut has_terminated) {
                        return self.empty_cancelled_result(
                            plan.cost,
                            plan.heuristics_version,
                            start_time,
                        );
                    }

                    if let PhysicalStep::ExpandNeighbors { policy, .. } = step {
                        if !merged.seed_nodes.is_empty() {
                            let stage = RetrievalStage::GraphExpansion;
                            sink.on_event(RetrievalEvent::StageStarted { stage });

                            let source = GraphExpansionSource::new(
                                merged.seed_nodes.clone(),
                                policy.clone(),
                            );
                            let candidates = source.retrieve(self.context);
                            expansions_performed += candidates.len();
                            candidates_produced += candidates.len();

                            for c in &candidates {
                                sink.on_event(RetrievalEvent::CandidateFound(c.clone()));
                                sink.on_event(RetrievalEvent::ExplanationUpdated {
                                    node_id: c.node_id,
                                    explanation: RetrievalExplanation {
                                        evidence_list: c.explanation_fragments.clone(),
                                    },
                                });
                            }
                            merged.runs.push(candidates);

                            sink.on_event(RetrievalEvent::StageCompleted { stage });
                        }
                    }
                }
            }
        }

        if self.check_cancel(cancellation, sink, &mut has_terminated) {
            return self.empty_cancelled_result(plan.cost, plan.heuristics_version, start_time);
        }

        // Fusion Stage
        let stage_fusion = RetrievalStage::Fusion;
        sink.on_event(RetrievalEvent::StageStarted {
            stage: stage_fusion,
        });
        let fused = self.fusion_strategy.fuse(&merged.runs);
        let candidates_fused = fused.len();
        sink.on_event(RetrievalEvent::StageCompleted {
            stage: stage_fusion,
        });

        if self.check_cancel(cancellation, sink, &mut has_terminated) {
            return self.empty_cancelled_result(plan.cost, plan.heuristics_version, start_time);
        }

        // Ranking Stage
        let stage_ranking = RetrievalStage::Ranking;
        sink.on_event(RetrievalEvent::StageStarted {
            stage: stage_ranking,
        });
        let (candidates, explanations) = self.ranking_strategy.rank(&fused);
        let ranking_operations = candidates.len();
        sink.on_event(RetrievalEvent::StageCompleted {
            stage: stage_ranking,
        });

        let elapsed_microseconds = start_time.elapsed().as_micros() as u64;

        let runtime = crate::retrieval::models::RuntimeMetadata {
            elapsed_microseconds,
            candidates_produced,
            candidates_fused,
            expansions_performed,
            ranking_operations,
        };

        let report = crate::retrieval::models::RetrievalExecutionReport {
            planning: crate::retrieval::models::PlanningMetadata {
                estimated_cost: plan.cost,
                planner_decisions,
                optimizer_decisions,
                heuristics_version: plan.heuristics_version,
            },
            runtime,
        };

        let result = RetrievalResult {
            candidates,
            explanations,
            report,
        };

        sink.on_event(RetrievalEvent::Completed {
            reason: CompletionReason::Finished,
            result: result.clone(),
        });

        result
    }

    /// Construct a helper empty result for cancellation.
    fn empty_cancelled_result(
        &self,
        cost: crate::retrieval::models::EstimatedCost,
        heuristics_version: u64,
        start_time: std::time::Instant,
    ) -> RetrievalResult {
        RetrievalResult {
            candidates: vec![],
            explanations: std::collections::HashMap::new(),
            report: crate::retrieval::models::RetrievalExecutionReport {
                planning: crate::retrieval::models::PlanningMetadata {
                    estimated_cost: cost,
                    planner_decisions: vec![],
                    optimizer_decisions: vec![],
                    heuristics_version,
                },
                runtime: crate::retrieval::models::RuntimeMetadata {
                    elapsed_microseconds: start_time.elapsed().as_micros() as u64,
                    candidates_produced: 0,
                    candidates_fused: 0,
                    expansions_performed: 0,
                    ranking_operations: 0,
                },
            },
        }
    }
}
