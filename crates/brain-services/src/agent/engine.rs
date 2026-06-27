use std::sync::Arc;
use std::time::Instant;

use brain_core::errors::BrainError;
use brain_core::extensibility::CancellationToken;
use brain_core::services::RetrievalService;
use brain_domain::{ConversationId, SessionId};
use brain_tools::CancellationTokenImpl;

use crate::agent::{
    AgentExecutionEventPayload, AgentToolExecutor, DefaultEventSink, ExecutionContext,
    ExecutionHandle, ExecutionId, ExecutionPolicy, ExecutionResult, ExecutionState,
    ExecutionStatus, ExecutionStep, MemoryCommit, MemoryCommitService, StageOutcome,
};

/// Internal trait representing a modular execution phase.
pub trait ExecutionStage: Send + Sync {
    /// Returns the name of this stage.
    fn name(&self) -> &'static str;

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

    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        if ctx.cancellation.is_cancelled() {
            return Ok(StageOutcome::Cancelled);
        }

        match ctx.retrieval.retrieve(&ctx.session_id, &ctx.prompt, 10) {
            Ok(nodes) => {
                state.retrieved_memories = nodes;
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

    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        if ctx.cancellation.is_cancelled() {
            return Ok(StageOutcome::Cancelled);
        }

        match ctx.chat.chat(ctx.session_id, &ctx.prompt) {
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

    fn execute(
        &self,
        ctx: &ExecutionContext,
        state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        if ctx.cancellation.is_cancelled() {
            return Ok(StageOutcome::Cancelled);
        }

        let user_msg = brain_domain::Message::new(
            brain_domain::MessageId::new(),
            brain_domain::MessageRole::User,
            ctx.prompt.clone(),
        );
        let assistant_msg = brain_domain::Message::new(
            brain_domain::MessageId::new(),
            brain_domain::MessageRole::Assistant,
            state.response_text.clone(),
        );

        state.pending_commit = MemoryCommit::new(vec![], vec![], vec![user_msg, assistant_msg]);

        ctx.commit_service
            .commit(&ctx.session_id, state.pending_commit.clone())?;

        ctx.sink.emit(AgentExecutionEventPayload::MemoryCommitted {
            session_id: ctx.session_id,
            node_count: 0,
            edge_count: 0,
        });

        Ok(StageOutcome::Finish)
    }
}

/// Coordinates and executes stages sequentially.
pub struct ExecutionRunner {
    stages: Vec<Box<dyn ExecutionStage>>,
}

impl ExecutionRunner {
    /// Creates a runner initialized with standard execution stages.
    pub fn new() -> Self {
        Self {
            stages: vec![
                Box::new(PlanningStage),
                Box::new(RetrievalStage),
                Box::new(ToolStage),
                Box::new(ReasoningStage),
                Box::new(CommitStage),
            ],
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

        for stage in &self.stages {
            if ctx.cancellation.is_cancelled() {
                return Err(BrainError::Cancelled {
                    message: "Execution cancelled".to_string(),
                });
            }

            let start = Instant::now();
            let outcome = stage.execute(ctx, state)?;
            steps.push(ExecutionStep {
                stage_name: stage.name(),
                duration_ms: start.elapsed().as_millis() as u64,
            });

            match outcome {
                StageOutcome::Continue => {}
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
    retrieval: Arc<dyn RetrievalService>,
    commit_service: Arc<dyn MemoryCommitService>,
    tool_executor: Arc<dyn AgentToolExecutor>,
    planner: Arc<dyn brain_core::agents::PlannerAgent>,
    chat: Arc<dyn brain_core::agents::ChatAgent>,
}

impl AgentExecutionEngine {
    /// Creates a new `AgentExecutionEngine`.
    pub fn new(
        policy: ExecutionPolicy,
        retrieval: Arc<dyn RetrievalService>,
        commit_service: Arc<dyn MemoryCommitService>,
        tool_executor: Arc<dyn AgentToolExecutor>,
        planner: Arc<dyn brain_core::agents::PlannerAgent>,
        chat: Arc<dyn brain_core::agents::ChatAgent>,
    ) -> Self {
        Self {
            policy,
            retrieval,
            commit_service,
            tool_executor,
            planner,
            chat,
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

        let sink = Arc::new(DefaultEventSink::new(execution_id, tx));
        let deadline = Some(Instant::now() + self.policy.timeout);

        let ctx = ExecutionContext {
            execution_id,
            session_id,
            conversation_id,
            prompt: prompt.to_string(),
            policy: self.policy.clone(),
            deadline,
            retrieval: self.retrieval.clone(),
            commit_service: self.commit_service.clone(),
            tool_executor: self.tool_executor.clone(),
            planner: self.planner.clone(),
            chat: self.chat.clone(),
            sink: sink.clone(),
            cancellation: cancellation.clone(),
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
                        ctx.sink
                            .emit(AgentExecutionEventPayload::ExecutionFinished {
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
                ctx.sink
                    .emit(AgentExecutionEventPayload::ExecutionCancelled {
                        session_id: ctx.session_id,
                    });
            }

            let _ = result_tx.send(res);
        });

        ExecutionHandle::new(status, cancellation, rx, result_rx)
    }
}
