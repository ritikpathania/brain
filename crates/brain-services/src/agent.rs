use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use brain_core::errors::BrainError;
use brain_core::extensibility::DecisionEngine;
use brain_domain::{ConversationId, Edge, MemoryDTO, Message, Node, SessionId};
use brain_tools::CancellationTokenImpl;

/// Strongly typed identifier for a single execution loop invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExecutionId(uuid::Uuid);

impl ExecutionId {
    /// Creates a new, globally unique `ExecutionId`.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Symbolic identifier for execution stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageIdentifier {
    /// Step planning.
    Planning,
    /// Memory retrieval.
    Retrieval,
    /// Running tools.
    ToolExecution,
    /// LLM inference.
    Reasoning,
    /// Outputs validation.
    Reflection,
    /// Safety checks.
    Verification,
    /// Changes persist.
    Commit,
}

/// Represents the control flow outcome of an execution stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageOutcome {
    /// Proceed to the next sequential stage.
    Continue,
    /// Finish the entire execution loop immediately.
    Finish,
    /// The stage execution has been cancelled.
    Cancelled,
    /// Request a reasoning retry loop.
    Retry {
        /// Feedback string to guide the next reasoning iteration.
        feedback: String,
        /// Target stage identifier.
        target: StageIdentifier,
    },
}

/// Outcome of reflection evaluations.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum ReflectionOutcome {
    /// Content accepted.
    Accept,
    /// Content requires correction retry.
    Retry {
        /// Feedback string for correction.
        feedback: String,
    },
}

/// Outcome of safety and fact verification.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum VerificationOutcome {
    /// Content passed verification.
    Passed,
    /// Content failed safety/correctness verification.
    Failed {
        /// Verification error descriptions.
        errors: Vec<String>,
    },
}

/// Context parameter passed to reflection policy evaluations.
#[derive(Debug, Clone)]
pub struct ReflectionContext {
    /// Original user text prompt.
    pub prompt: String,
    /// Latest generated text response.
    pub response: String,
}

/// Decision returned by reflection policy evaluations.
#[derive(Debug, Clone)]
pub struct ReflectionDecision {
    /// Evaluation outcome.
    pub outcome: ReflectionOutcome,
}

/// Context parameter passed to verification policy evaluations. Fully owned.
#[derive(Debug, Clone)]
pub struct VerificationContext {
    /// Original user text prompt.
    pub prompt: String,
    /// Latest generated text response.
    pub response: String,
    /// List of retrieved memories.
    pub retrieved_memories: Vec<MemoryDTO>,
    /// Key-value map of executed tool outputs.
    pub tool_outputs: HashMap<String, serde_json::Value>,
}

/// Decision returned by verification policy evaluations.
#[derive(Debug, Clone)]
pub struct VerificationDecision {
    /// Safety/Correctness outcome.
    pub outcome: VerificationOutcome,
    /// Calculated confidence score.
    pub confidence_score: f32,
}

/// Execution constraints and iteration limits.
#[derive(Debug, Clone)]
pub struct ExecutionPolicy {
    /// Max iterations for the tool execution loop.
    pub max_iterations: usize,
    /// Max retry attempts for tools (retained for future API stability).
    pub max_tool_retries: usize,
    /// Absolute timeout limit for the request loop.
    pub timeout: Duration,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            max_tool_retries: 3,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Telemetry metrics for execution tracking, aggregated by the event sink.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ExecutionMetrics {
    /// Number of tokens processed or generated (approximate word count).
    pub tokens_used: usize,
    /// Total execution steps/actions run.
    pub step_count: usize,
    /// Execution loop duration in milliseconds.
    pub duration_ms: u64,
}

/// Description of an executed execution stage.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionStep {
    /// Name of the stage executed.
    pub stage_name: &'static str,
    /// Duration of the stage execution in milliseconds.
    pub duration_ms: u64,
}

/// Immutable result carrying final outcome information, constructed upon completion.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    response_text: String,
    steps: Vec<ExecutionStep>,
    metrics: ExecutionMetrics,
}

impl ExecutionResult {
    /// Creates a new immutable `ExecutionResult`.
    pub fn new(
        response_text: String,
        steps: Vec<ExecutionStep>,
        metrics: ExecutionMetrics,
    ) -> Self {
        Self {
            response_text,
            steps,
            metrics,
        }
    }

    /// Returns the final generated response text.
    pub fn response_text(&self) -> &str {
        &self.response_text
    }

    /// Returns the historical steps run.
    pub fn steps(&self) -> &[ExecutionStep] {
        &self.steps
    }

    /// Returns aggregated metrics.
    pub fn metrics(&self) -> &ExecutionMetrics {
        &self.metrics
    }
}

/// Overall status of the active execution session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ExecutionStatus {
    /// The engine execution is actively running.
    Running,
    /// The engine completed successfully.
    Succeeded,
    /// The engine encountered an unrecoverable failure.
    Failed,
    /// The execution was aborted by cancellation.
    Cancelled,
}

/// Batch transactional memory updates to commit atomically.
#[derive(Debug, Clone, Default)]
pub struct MemoryCommit {
    /// The new nodes to save.
    pub nodes: Vec<Node>,
    /// The new edges to save.
    pub edges: Vec<Edge>,
    /// The conversation history messages to append.
    pub messages: Vec<Message>,
}

impl MemoryCommit {
    /// Creates a new batch commit.
    pub fn new(nodes: Vec<Node>, edges: Vec<Edge>, messages: Vec<Message>) -> Self {
        Self {
            nodes,
            edges,
            messages,
        }
    }
}

/// Service responsible for transactional commits.
pub trait MemoryCommitService: Send + Sync {
    /// Commits all updates in a single database transaction.
    fn commit(&self, session_id: &SessionId, commit: MemoryCommit) -> Result<(), BrainError>;
}

/// Decoupled interface for tool execution.
pub trait AgentToolExecutor: Send + Sync {
    /// Executes a single registered tool.
    fn execute(
        &self,
        session_id: &SessionId,
        tool_name: &str,
        arguments: &HashMap<String, serde_json::Value>,
        deadline: Option<Instant>,
    ) -> Result<brain_core::extensibility::ExecutionResult, BrainError>;
}

/// Monotonic, timestamped events produced during execution.
#[derive(Debug, Clone, serde::Serialize)]
pub enum AgentExecutionEventPayload {
    /// Started execution.
    ExecutionStarted {
        /// Target session identifier.
        session_id: SessionId,
        /// Original user text prompt.
        prompt: String,
    },
    /// Planning has started.
    PlanningStarted {
        /// Target session identifier.
        session_id: SessionId,
    },
    /// Retrieval completed.
    RetrievalCompleted {
        /// Target session identifier.
        session_id: SessionId,
        /// Match count.
        match_count: usize,
    },
    /// A tool was invoked.
    ToolInvoked {
        /// Target session identifier.
        session_id: SessionId,
        /// Name of the tool.
        tool_name: String,
        /// Arguments.
        arguments: HashMap<String, serde_json::Value>,
    },
    /// A tool completed execution.
    ToolCompleted {
        /// Target session identifier.
        session_id: SessionId,
        /// Name of the tool.
        tool_name: String,
        /// Execution output value.
        result: serde_json::Value,
    },
    /// Graph memory changes committed.
    MemoryCommitted {
        /// Target session identifier.
        session_id: SessionId,
        /// Committed nodes count.
        node_count: usize,
        /// Committed edges count.
        edge_count: usize,
    },
    /// Streamed token emitted.
    TokenStreamed {
        /// Target session identifier.
        session_id: SessionId,
        /// Individual word token.
        token: String,
    },
    /// Completed successfully.
    ExecutionFinished {
        /// Target session identifier.
        session_id: SessionId,
        /// Final generated text response.
        response: String,
    },
    /// Encountered failure.
    ExecutionFailed {
        /// Target session identifier.
        session_id: SessionId,
        /// Error details.
        error: String,
    },
    /// Terminated via cancellation.
    ExecutionCancelled {
        /// Target session identifier.
        session_id: SessionId,
    },
    /// Reflection step completed.
    ReflectionEvaluated {
        /// Target session identifier.
        session_id: SessionId,
        /// Evaluation outcome.
        outcome: ReflectionOutcome,
    },
    /// Verification step completed.
    VerificationCompleted {
        /// Target session identifier.
        session_id: SessionId,
        /// Safety/Correctness outcome.
        outcome: VerificationOutcome,
        /// Calculated confidence score.
        confidence_score: f32,
    },
    /// A stage has started execution.
    StageStarted {
        /// Target session identifier.
        session_id: SessionId,
        /// Name of the stage.
        stage: &'static str,
    },
    /// A stage has completed execution.
    StageCompleted {
        /// Target session identifier.
        session_id: SessionId,
        /// Name of the stage.
        stage: &'static str,
        /// Execution duration in milliseconds.
        duration_ms: u64,
    },
}

/// Wrapped execution event enriched with metadata inside the event sink.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentExecutionEvent {
    /// Unique execution identifier.
    pub execution_id: ExecutionId,
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// SystemTime timestamp.
    pub timestamp: SystemTime,
    /// Specific event metadata payload.
    pub payload: AgentExecutionEventPayload,
}

/// Decoupled interface for event logging, metrics aggregation, and streaming.
pub trait ExecutionEventSink: Send + Sync {
    /// Emits a raw event payload, timestamps it, assigns sequence, and aggregates metrics.
    fn emit(&self, payload: AgentExecutionEventPayload);
    /// Returns aggregated metrics.
    fn metrics(&self) -> ExecutionMetrics;
}

/// Concrete implementation of `ExecutionEventSink` feeding an unbounded mpsc channel.
pub struct DefaultEventSink {
    execution_id: ExecutionId,
    sequence: std::sync::atomic::AtomicU64,
    start_time: Instant,
    tx: tokio::sync::mpsc::UnboundedSender<AgentExecutionEvent>,
    metrics: Arc<parking_lot::Mutex<ExecutionMetrics>>,
}

impl DefaultEventSink {
    /// Creates a new `DefaultEventSink` correlating events with the execution ID.
    pub fn new(
        execution_id: ExecutionId,
        tx: tokio::sync::mpsc::UnboundedSender<AgentExecutionEvent>,
    ) -> Self {
        Self {
            execution_id,
            sequence: std::sync::atomic::AtomicU64::new(1),
            start_time: Instant::now(),
            tx,
            metrics: Arc::new(parking_lot::Mutex::new(ExecutionMetrics::default())),
        }
    }
}

impl ExecutionEventSink for DefaultEventSink {
    fn emit(&self, payload: AgentExecutionEventPayload) {
        let seq = self
            .sequence
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let now_sys = SystemTime::now();
        let event = AgentExecutionEvent {
            execution_id: self.execution_id,
            sequence: seq,
            timestamp: now_sys,
            payload: payload.clone(),
        };

        // Accumulate metrics inside the sink
        {
            let mut m = self.metrics.lock();
            m.duration_ms = self.start_time.elapsed().as_millis() as u64;
            match &payload {
                AgentExecutionEventPayload::ToolCompleted { .. } => {
                    m.step_count += 1;
                }
                AgentExecutionEventPayload::TokenStreamed { token, .. } => {
                    m.tokens_used += token.split_whitespace().count().max(1);
                }
                _ => {}
            }
        }

        let _ = self.tx.send(event);
    }

    fn metrics(&self) -> ExecutionMetrics {
        let mut m = self.metrics.lock().clone();
        m.duration_ms = self.start_time.elapsed().as_millis() as u64;
        m
    }
}

/// Immutable context holding injected service interfaces and execution parameters.
pub struct ExecutionContext {
    /// Core execution identifier.
    pub execution_id: ExecutionId,
    /// Target session identifier.
    pub session_id: SessionId,
    /// Conversation identifier.
    pub conversation_id: ConversationId,
    /// Original user text prompt.
    pub prompt: String,
    /// Execution policy boundaries.
    pub policy: ExecutionPolicy,
    /// Time deadline limit.
    pub deadline: Option<Instant>,
    /// Injected conversation manager.
    pub conversation_manager: Arc<dyn crate::conversation::ConversationManager>,
    /// Injected tool executor coordinator.
    pub tool_executor: Arc<dyn AgentToolExecutor>,
    /// Injected step planner agent.
    pub planner: Arc<dyn brain_core::agents::PlannerAgent>,
    /// Injected chat model reasoning agent.
    pub chat: Arc<dyn brain_core::agents::ChatAgent>,
    /// Event output logging sink.
    pub sink: Arc<dyn ExecutionEventSink>,
    /// Thread cancellation listener.
    pub cancellation: Arc<CancellationTokenImpl>,
    /// Injected reflection engine.
    pub reflection_engine: Arc<dyn DecisionEngine<ReflectionContext, ReflectionDecision>>,
    /// Injected verification engine.
    pub verification_engine: Arc<dyn DecisionEngine<VerificationContext, VerificationDecision>>,
}

/// Mutable state representation modified progressively by sequential execution stages.
#[derive(Debug, Clone, Default)]
pub struct ExecutionState {
    /// Contextual memories loaded.
    pub retrieved_memories: Vec<MemoryDTO>,
    /// Decided tool plan calls.
    pub planner_output: Vec<brain_domain::ToolCall>,
    /// Running output maps.
    pub tool_outputs: HashMap<String, serde_json::Value>,
    /// Tokens streamed.
    pub streamed_tokens: Vec<String>,
    /// Final response text string.
    pub response_text: String,
    /// Pending memory updates.
    pub pending_commit: MemoryCommit,
    /// Pending feedback for self-correction retries.
    pub feedback_prompt: Option<String>,
}

impl ExecutionState {
    /// Creates a default, empty `ExecutionState`.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Handle exposing execution status, cancellation trigger, and blocking wait.
pub struct ExecutionHandle {
    execution_id: ExecutionId,
    status: Arc<parking_lot::RwLock<ExecutionStatus>>,
    cancellation: Arc<CancellationTokenImpl>,
    result_rx: tokio::sync::oneshot::Receiver<Result<ExecutionResult, BrainError>>,
}

impl ExecutionHandle {
    /// Creates a new `ExecutionHandle`.
    pub fn new(
        execution_id: ExecutionId,
        status: Arc<parking_lot::RwLock<ExecutionStatus>>,
        cancellation: Arc<CancellationTokenImpl>,
        result_rx: tokio::sync::oneshot::Receiver<Result<ExecutionResult, BrainError>>,
    ) -> Self {
        Self {
            execution_id,
            status,
            cancellation,
            result_rx,
        }
    }

    /// Returns the execution identifier.
    pub fn execution_id(&self) -> ExecutionId {
        self.execution_id
    }

    /// Returns the current execution status.
    pub fn status(&self) -> ExecutionStatus {
        *self.status.read()
    }

    /// Triggers cooperative cancellation of the execution thread. Idempotent.
    pub fn cancel(&self) {
        self.cancellation.cancel();
        let mut status = self.status.write();
        if *status == ExecutionStatus::Running {
            *status = ExecutionStatus::Cancelled;
        }
    }

    /// Awaits the final execution result.
    pub async fn wait(self) -> Result<ExecutionResult, BrainError> {
        self.result_rx.await.map_err(|_| BrainError::Internal {
            message: "Execution thread terminated prematurely".to_string(),
        })?
    }
}

/// Submodule implementing commit logic.
pub mod commit;
/// Submodule implementing runner and engine logic.
pub mod engine;
/// Submodule implementing runtime streaming logic.
pub mod streaming;
/// Submodule implementing graph-based workflow execution.
pub mod graph;
