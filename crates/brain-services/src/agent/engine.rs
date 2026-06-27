use std::sync::Arc;
use std::time::Instant;

use brain_core::errors::BrainError;
use brain_core::extensibility::CancellationToken;
use brain_domain::{ConversationId, SessionId};
use brain_tools::CancellationTokenImpl;

use brain_core::extensibility::DecisionEngine;
use crate::agent::{
    AgentExecutionEventPayload, AgentToolExecutor, DefaultEventSink, ExecutionContext,
    ExecutionHandle, ExecutionId, ExecutionPolicy, ExecutionResult, ExecutionState,
    ExecutionStatus, ExecutionStep, StageOutcome, StageIdentifier, ReflectionContext,
    ReflectionOutcome, ReflectionDecision, VerificationContext, VerificationOutcome,
    VerificationDecision,
};
use crate::agent::streaming::StreamingRuntime;
use crate::conversation::ConversationManager;
/// Interface for replaceable reflection policies.
pub trait ReflectionPolicy: Send + Sync {
    /// Evaluates generated output against reflection criteria.
    fn evaluate(&self, ctx: &ReflectionContext) -> Result<ReflectionDecision, BrainError>;
}

/// Interface for safety and content verification policies.
pub trait VerificationPolicy: Send + Sync {
    /// Verifies content safety and truthfulness.
    fn verify(&self, ctx: &VerificationContext) -> Result<bool, BrainError>;
}

/// Interface for calculating heuristic confidence metrics.
pub trait ConfidencePolicy: Send + Sync {
    /// Calculates a confidence score in range [0.0, 1.0].
    fn calculate_confidence(&self, ctx: &VerificationContext) -> Result<f32, BrainError>;
}

/// Concrete reflection engine implementation.
pub struct ReflectionEngineImpl<P> {
    /// Injected reflection policy.
    pub policy: P,
}

impl<P: ReflectionPolicy> DecisionEngine<ReflectionContext, ReflectionDecision> for ReflectionEngineImpl<P> {
    fn evaluate(&self, context: &ReflectionContext) -> Result<ReflectionDecision, BrainError> {
        self.policy.evaluate(context)
    }
}

/// Concrete verification engine implementation.
pub struct VerificationEngineImpl<PV, PC> {
    /// Injected safety verification policy.
    pub verification_policy: PV,
    /// Injected confidence calculation policy.
    pub confidence_policy: PC,
}

impl<PV: VerificationPolicy, PC: ConfidencePolicy> VerificationEngineImpl<PV, PC> {
    /// Creates a new `VerificationEngineImpl`.
    pub fn new(verification_policy: PV, confidence_policy: PC) -> Self {
        Self {
            verification_policy,
            confidence_policy,
        }
    }
}

impl<PV: VerificationPolicy, PC: ConfidencePolicy> DecisionEngine<VerificationContext, VerificationDecision> for VerificationEngineImpl<PV, PC> {
    fn evaluate(&self, context: &VerificationContext) -> Result<VerificationDecision, BrainError> {
        let verified = self.verification_policy.verify(context)?;
        let confidence_score = self.confidence_policy.calculate_confidence(context)?;
        let outcome = if verified {
            VerificationOutcome::Passed
        } else {
            VerificationOutcome::Failed {
                errors: vec!["Content safety verification failed".to_string()],
            }
        };
        Ok(VerificationDecision {
            outcome,
            confidence_score,
        })
    }
}

/// Concrete reflection policy detecting TODO placeholders.
pub struct RegexReflectionPolicy {
    /// Search string that is forbidden.
    pub forbidden_pattern: String,
}

impl ReflectionPolicy for RegexReflectionPolicy {
    fn evaluate(&self, ctx: &ReflectionContext) -> Result<ReflectionDecision, BrainError> {
        if ctx.response.contains(&self.forbidden_pattern) {
            Ok(ReflectionDecision {
                outcome: ReflectionOutcome::Retry {
                    feedback: format!("Please rewrite without using '{}'", self.forbidden_pattern),
                },
            })
        } else {
            Ok(ReflectionDecision {
                outcome: ReflectionOutcome::Accept,
            })
        }
    }
}

/// Concrete safety verification policy.
pub struct SafeVerificationPolicy;

impl VerificationPolicy for SafeVerificationPolicy {
    fn verify(&self, ctx: &VerificationContext) -> Result<bool, BrainError> {
        Ok(!ctx.response.to_lowercase().contains("unsafe"))
    }
}

/// Concrete confidence score calculator based on memory availability.
pub struct HeuristicConfidencePolicy;

impl ConfidencePolicy for HeuristicConfidencePolicy {
    fn calculate_confidence(&self, ctx: &VerificationContext) -> Result<f32, BrainError> {
        if !ctx.retrieved_memories.is_empty() {
            Ok(0.9)
        } else {
            Ok(0.6)
        }
    }
}

/// Internal trait representing a modular execution phase.
pub trait ExecutionStage: Send + Sync {
    /// Returns the name of this stage.
    fn name(&self) -> &'static str;

    /// Returns the symbolic identifier of this stage.
    fn id(&self) -> StageIdentifier;

    /// Returns true if this stage supports self-correction retry loops.
    fn supports_retry(&self) -> bool {
        false
    }

    /// Executes the stage, progressively modifying the mutable state.
    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError>;
}

/// Sequential planner parsing and dispatching phase.
pub struct PlanningStage;

impl ExecutionStage for PlanningStage {
    fn name(&self) -> &'static str {
        "Planning"
    }

    fn id(&self) -> StageIdentifier {
        StageIdentifier::Planning
    }

    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        ctx.sink.emit(AgentExecutionEventPayload::PlanningStarted {
            session_id: ctx.session_id,
        });

        if ctx.cancellation.is_cancelled() {
            return Ok(StageOutcome::Cancelled);
        }

        // Context history resolves context conversation
        let history = brain_domain::Conversation::new_empty();
        match ctx.planner.plan_steps(&ctx.prompt, &history) {
            Ok(steps) => {
                state.planner_output = steps;
                Ok(StageOutcome::Continue)
            }
            Err(e) => Err(e),
        }
    }
}

/// Ephemeral and Persistent memory retrieval stage.
pub struct RetrievalStage;

impl ExecutionStage for RetrievalStage {
    fn name(&self) -> &'static str {
        "Retrieval"
    }

    fn id(&self) -> StageIdentifier {
        StageIdentifier::Retrieval
    }

    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        if ctx.cancellation.is_cancelled() {
            return Ok(StageOutcome::Cancelled);
        }

        let budget = crate::conversation::ContextBudget {
            max_tokens: 4096,
            reserved_system_tokens: 512,
            reserved_completion_tokens: 512,
        };

        match ctx
            .conversation_manager
            .build_context_window(&ctx.session_id, budget)
        {
            Ok(window) => {
                state.retrieved_memories = window.retrieved_memories().to_vec();
                ctx.sink
                    .emit(AgentExecutionEventPayload::RetrievalCompleted {
                        session_id: ctx.session_id,
                        match_count: state.retrieved_memories.len(),
                    });
                Ok(StageOutcome::Continue)
            }
            Err(e) => Err(e),
        }
    }
}

/// Loop iteration phase running agent requested tools.
pub struct ToolStage;

impl ExecutionStage for ToolStage {
    fn name(&self) -> &'static str {
        "ToolExecution"
    }

    fn id(&self) -> StageIdentifier {
        StageIdentifier::ToolExecution
    }

    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        let planner_steps = state.planner_output.clone();

        for tool_call in planner_steps.into_iter().take(ctx.policy.max_iterations) {
            if ctx.cancellation.is_cancelled() {
                return Ok(StageOutcome::Cancelled);
            }

            ctx.sink.emit(AgentExecutionEventPayload::ToolInvoked {
                session_id: ctx.session_id,
                tool_name: tool_call.tool_name.clone(),
                arguments: tool_call.arguments.clone(),
            });

            match ctx.tool_executor.execute(
                &ctx.session_id,
                &tool_call.tool_name,
                &tool_call.arguments,
                ctx.deadline,
            ) {
                Ok(res) => {
                    state
                        .tool_outputs
                        .insert(tool_call.tool_name.clone(), res.value().clone());
                    ctx.sink.emit(AgentExecutionEventPayload::ToolCompleted {
                        session_id: ctx.session_id,
                        tool_name: tool_call.tool_name.clone(),
                        result: res.value().clone(),
                    });
                }
                Err(e) => return Err(e),
            }
        }
        Ok(StageOutcome::Continue)
    }
}

/// LLM inference reasoning and word token streaming stage.
pub struct ReasoningStage;

impl ExecutionStage for ReasoningStage {
    fn name(&self) -> &'static str {
        "Reasoning"
    }

    fn id(&self) -> StageIdentifier {
        StageIdentifier::Reasoning
    }

    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        if ctx.cancellation.is_cancelled() {
            return Ok(StageOutcome::Cancelled);
        }

        let prompt = if let Some(ref fb) = state.feedback_prompt {
            format!("Original Prompt: {}\nFeedback: {}\nCorrected Response:", ctx.prompt, fb)
        } else {
            ctx.prompt.clone()
        };

        match ctx.chat.chat(ctx.session_id, &prompt) {
            Ok(response) => {
                state.response_text = response.clone();
                // Stream tokens word-by-word
                let words: Vec<&str> = response.split_whitespace().collect();
                for word in words {
                    if ctx.cancellation.is_cancelled() {
                        return Ok(StageOutcome::Cancelled);
                    }
                    ctx.sink.emit(AgentExecutionEventPayload::TokenStreamed {
                        session_id: ctx.session_id,
                        token: format!("{} ", word),
                    });
                }
                Ok(StageOutcome::Continue)
            }
            Err(e) => Err(e),
        }
    }
}

/// Transactional memory update and commit stage.
pub struct CommitStage;

impl ExecutionStage for CommitStage {
    fn name(&self) -> &'static str {
        "Commit"
    }

    fn id(&self) -> StageIdentifier {
        StageIdentifier::Commit
    }

    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        if ctx.cancellation.is_cancelled() {
            return Ok(StageOutcome::Cancelled);
        }

        let policy = crate::conversation::IngestionPolicy { stm_only: true };

        ctx.conversation_manager.ingest_interaction(
            &ctx.session_id,
            &ctx.prompt,
            &state.response_text,
            policy,
        )?;

        ctx.sink.emit(AgentExecutionEventPayload::MemoryCommitted {
            session_id: ctx.session_id,
            node_count: 0,
            edge_count: 0,
        });

        Ok(StageOutcome::Finish)
    }
}

/// Stage checking reasoning outputs for necessary self-corrections.
pub struct ReflectionStage;

impl ExecutionStage for ReflectionStage {
    fn name(&self) -> &'static str {
        "Reflection"
    }

    fn id(&self) -> StageIdentifier {
        StageIdentifier::Reflection
    }

    fn supports_retry(&self) -> bool {
        true
    }

    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        let ref_ctx = ReflectionContext {
            prompt: ctx.prompt.clone(),
            response: state.response_text.clone(),
        };

        let decision = ctx.reflection_engine.evaluate(&ref_ctx)?;

        ctx.sink.emit(AgentExecutionEventPayload::ReflectionEvaluated {
            session_id: ctx.session_id,
            outcome: decision.outcome.clone(),
        });

        match decision.outcome {
            ReflectionOutcome::Accept => Ok(StageOutcome::Continue),
            ReflectionOutcome::Retry { feedback } => Ok(StageOutcome::Retry {
                feedback,
                target: StageIdentifier::Reasoning,
            }),
        }
    }
}

/// Stage verifying content safety and output confidence.
pub struct VerificationStage;

impl ExecutionStage for VerificationStage {
    fn name(&self) -> &'static str {
        "Verification"
    }

    fn id(&self) -> StageIdentifier {
        StageIdentifier::Verification
    }

    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        let ver_ctx = VerificationContext {
            prompt: ctx.prompt.clone(),
            response: state.response_text.clone(),
            retrieved_memories: state.retrieved_memories.clone(),
            tool_outputs: state.tool_outputs.clone(),
        };

        let decision = ctx.verification_engine.evaluate(&ver_ctx)?;

        ctx.sink.emit(AgentExecutionEventPayload::VerificationCompleted {
            session_id: ctx.session_id,
            outcome: decision.outcome.clone(),
            confidence_score: decision.confidence_score,
        });

        match decision.outcome {
            VerificationOutcome::Passed => Ok(StageOutcome::Continue),
            VerificationOutcome::Failed { errors } => {
                let err_msg = format!("Verification failed: {:?}", errors);
                Err(BrainError::Validation { message: err_msg })
            }
        }
    }
}

/// Coordinates and executes stages sequentially.
pub struct ExecutionRunner {
    stages: Vec<Box<dyn ExecutionStage>>,
    stage_indices: std::collections::HashMap<StageIdentifier, usize>,
}

impl ExecutionRunner {
    /// Creates a runner initialized with standard execution stages.
    pub fn new() -> Self {
        let stages: Vec<Box<dyn ExecutionStage>> = vec![
            Box::new(PlanningStage),
            Box::new(RetrievalStage),
            Box::new(ToolStage),
            Box::new(ReasoningStage),
            Box::new(ReflectionStage),
            Box::new(VerificationStage),
            Box::new(CommitStage),
        ];

        let mut stage_indices = std::collections::HashMap::new();
        for (idx, stage) in stages.iter().enumerate() {
            stage_indices.insert(stage.id(), idx);
        }

        Self {
            stages,
            stage_indices,
        }
    }

    /// Executes sequential stages on state using context parameters.
    pub fn run(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<ExecutionResult, BrainError> {
        let mut steps = Vec::new();
        ctx.sink.emit(AgentExecutionEventPayload::ExecutionStarted {
            session_id: ctx.session_id,
            prompt: ctx.prompt.clone(),
        });

        let mut stage_index = 0;
        let mut attempts = 0;
        let max_attempts = 3;

        while stage_index < self.stages.len() {
            let stage = &self.stages[stage_index];

            if ctx.cancellation.is_cancelled() {
                return Err(BrainError::Cancelled {
                    message: "Execution cancelled".to_string(),
                });
            }

            ctx.sink.emit(AgentExecutionEventPayload::StageStarted {
                session_id: ctx.session_id,
                stage: stage.name(),
            });

            let start = Instant::now();
            let outcome = stage.execute(ctx, state)?;
            let duration_ms = start.elapsed().as_millis() as u64;

            ctx.sink.emit(AgentExecutionEventPayload::StageCompleted {
                session_id: ctx.session_id,
                stage: stage.name(),
                duration_ms,
            });

            steps.push(ExecutionStep {
                stage_name: stage.name(),
                duration_ms,
            });

            match outcome {
                StageOutcome::Continue => {
                    stage_index += 1;
                }
                StageOutcome::Retry { feedback, target } => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(BrainError::Validation {
                            message: format!("Self-correction failed after {} attempts.", max_attempts),
                        });
                    }
                    if let Some(&idx) = self.stage_indices.get(&target) {
                        state.feedback_prompt = Some(feedback);
                        stage_index = idx;
                    } else {
                        stage_index += 1;
                    }
                }
                StageOutcome::Finish => {
                    break;
                }
                StageOutcome::Cancelled => {
                    return Err(BrainError::Cancelled {
                        message: "Execution cancelled".to_string(),
                    });
                }
            }
        }

        let metrics = ctx.sink.metrics();
        Ok(ExecutionResult::new(
            state.response_text.clone(),
            steps,
            metrics,
        ))
    }
}

impl Default for ExecutionRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Orchestration coordinator responsible for user agent request loops.
pub struct AgentExecutionEngine {
    policy: ExecutionPolicy,
    conversation_manager: Arc<dyn ConversationManager>,
    tool_executor: Arc<dyn AgentToolExecutor>,
    planner: Arc<dyn brain_core::agents::PlannerAgent>,
    chat: Arc<dyn brain_core::agents::ChatAgent>,
    streaming_runtime: Arc<StreamingRuntime>,
    /// Injected reflection evaluation engine.
    pub reflection_engine: Arc<dyn DecisionEngine<ReflectionContext, ReflectionDecision>>,
    /// Injected safety verification engine.
    pub verification_engine: Arc<dyn DecisionEngine<VerificationContext, VerificationDecision>>,
}

impl AgentExecutionEngine {
    /// Creates a new `AgentExecutionEngine`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        policy: ExecutionPolicy,
        conversation_manager: Arc<dyn ConversationManager>,
        tool_executor: Arc<dyn AgentToolExecutor>,
        planner: Arc<dyn brain_core::agents::PlannerAgent>,
        chat: Arc<dyn brain_core::agents::ChatAgent>,
        streaming_runtime: Arc<StreamingRuntime>,
        reflection_engine: Arc<dyn DecisionEngine<ReflectionContext, ReflectionDecision>>,
        verification_engine: Arc<dyn DecisionEngine<VerificationContext, VerificationDecision>>,
    ) -> Self {
        Self {
            policy,
            conversation_manager,
            tool_executor,
            planner,
            chat,
            streaming_runtime,
            reflection_engine,
            verification_engine,
        }
    }

    /// Spawns the runner execution thread returning a control handle.
    pub fn execute(
        &self,
        session_id: SessionId,
        conversation_id: ConversationId,
        prompt: &str,
    ) -> ExecutionHandle {
        let execution_id = ExecutionId::new();
        let status = Arc::new(parking_lot::RwLock::new(ExecutionStatus::Running));
        let cancellation = Arc::new(CancellationTokenImpl::new());
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();

        self.streaming_runtime.register(execution_id, rx, cancellation.clone());

        let sink = Arc::new(DefaultEventSink::new(execution_id, tx));
        let deadline = Some(Instant::now() + self.policy.timeout);

        let ctx = ExecutionContext {
            execution_id,
            session_id,
            conversation_id,
            prompt: prompt.to_string(),
            policy: self.policy.clone(),
            deadline,
            conversation_manager: self.conversation_manager.clone(),
            tool_executor: self.tool_executor.clone(),
            planner: self.planner.clone(),
            chat: self.chat.clone(),
            sink: sink.clone(),
            cancellation: cancellation.clone(),
            reflection_engine: self.reflection_engine.clone(),
            verification_engine: self.verification_engine.clone(),
        };

        let runner = ExecutionRunner::new();
        let status_clone = status.clone();

        tokio::spawn(async move {
            let mut state = ExecutionState::new();
            let res = runner.run(&ctx, &mut state);

            let mut final_status = status_clone.write();
            if *final_status == ExecutionStatus::Running {
                match &res {
                    Ok(result) => {
                        *final_status = ExecutionStatus::Succeeded;
                        ctx.sink.emit(AgentExecutionEventPayload::ExecutionFinished {
                            session_id: ctx.session_id,
                            response: result.response_text().to_string(),
                        });
                    }
                    Err(e) => {
                        *final_status = ExecutionStatus::Failed;
                        ctx.sink.emit(AgentExecutionEventPayload::ExecutionFailed {
                            session_id: ctx.session_id,
                            error: e.to_string(),
                        });
                    }
                }
            } else if *final_status == ExecutionStatus::Cancelled {
                ctx.sink.emit(AgentExecutionEventPayload::ExecutionCancelled {
                    session_id: ctx.session_id,
                });
            }

            let _ = result_tx.send(res);
        });

        ExecutionHandle::new(execution_id, status, cancellation, result_rx)
    }
}
