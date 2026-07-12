use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use brain_core::errors::BrainError;
use brain_core::services::RetrievalService;
use brain_domain::{Session, ConversationId, MemoryDTO, SessionId, ToolCall};
use brain_services::agent::engine::AgentExecutionEngine;
use brain_services::agent::{
    AgentToolExecutor, ExecutionPolicy,
    ExecutionStatus, MemoryCommit, MemoryCommitService,
};
use brain_services::agent::streaming::{StreamingRuntime, DefaultStreamEventMapper};

fn create_default_test_engine(
    policy: ExecutionPolicy,
    conversation_manager: Arc<dyn brain_services::conversation::ConversationManager>,
    tool_executor: Arc<dyn AgentToolExecutor>,
    planner: Arc<dyn brain_core::agents::PlannerAgent>,
    chat: Arc<dyn brain_core::agents::ChatAgent>,
    streaming_runtime: Arc<StreamingRuntime>,
) -> AgentExecutionEngine {
    let reflection_engine = Arc::new(brain_services::agent::engine::ReflectionEngineImpl {
        policy: brain_services::agent::engine::RegexReflectionPolicy { forbidden_pattern: "TODO".to_string() },
    });
    let verification_engine = Arc::new(brain_services::agent::engine::VerificationEngineImpl::new(
        brain_services::agent::engine::SafeVerificationPolicy,
        brain_services::agent::engine::HeuristicConfidencePolicy,
    ));
    AgentExecutionEngine::new(
        policy,
        conversation_manager,
        tool_executor,
        planner,
        chat,
        streaming_runtime,
        reflection_engine,
        verification_engine,
    )
}

// --- Mock Implementations ---

struct MockPlanner {
    tool_calls: Vec<ToolCall>,
    should_fail: bool,
}

impl brain_core::agents::PlannerAgent for MockPlanner {
    fn name(&self) -> &str {
        "MockPlanner"
    }

    fn plan_steps(
        &self,
        _task: &str,
        _history: &Session,
    ) -> Result<Vec<ToolCall>, BrainError> {
        if self.should_fail {
            return Err(BrainError::Validation {
                message: "Planner failure".to_string(),
            });
        }
        Ok(self.tool_calls.clone())
    }
}

struct MockChat {
    response: String,
    should_fail: bool,
}

impl brain_core::agents::ChatAgent for MockChat {
    fn name(&self) -> &str {
        "MockChat"
    }

    fn chat(&self, _session_id: SessionId, _prompt: &str) -> Result<String, BrainError> {
        if self.should_fail {
            return Err(BrainError::Python {
                message: "LLM reasoning error".to_string(),
                traceback: None,
            });
        }
        Ok(self.response.clone())
    }
}

struct MockRetrieval {
    nodes: Vec<MemoryDTO>,
}

impl RetrievalService for MockRetrieval {
    fn retrieve(
        &self,
        _session_id: &SessionId,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<MemoryDTO>, BrainError> {
        Ok(self.nodes.clone())
    }
}

struct MockToolExecutor {
    should_fail: bool,
    latency: Duration,
    execution_count: Arc<AtomicUsize>,
}

impl AgentToolExecutor for MockToolExecutor {
    fn execute(
        &self,
        _session_id: &SessionId,
        tool_name: &str,
        _arguments: &HashMap<String, serde_json::Value>,
        _deadline: Option<Instant>,
    ) -> Result<brain_core::extensibility::ExecutionResult, BrainError> {
        self.execution_count.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            return Err(BrainError::Tool {
                tool_name: tool_name.to_string(),
                message: "Tool failed".to_string(),
            });
        }
        if self.latency > Duration::ZERO {
            std::thread::sleep(self.latency);
        }
        Ok(brain_core::extensibility::ExecutionResult::new(
            serde_json::json!({ "status": "success" }),
        ))
    }
}

struct MockCommitService {
    committed: Arc<AtomicBool>,
    should_fail: bool,
    committed_nodes: Arc<parking_lot::Mutex<Vec<brain_domain::Node>>>,
    committed_messages: Arc<parking_lot::Mutex<Vec<brain_domain::Message>>>,
}

impl MemoryCommitService for MockCommitService {
    fn commit(&self, _session_id: &SessionId, commit: MemoryCommit) -> Result<(), BrainError> {
        if self.should_fail {
            return Err(BrainError::Storage {
                message: "Storage commit failure".to_string(),
                source: None,
            });
        }
        self.committed.store(true, Ordering::SeqCst);
        *self.committed_nodes.lock() = commit.nodes;
        *self.committed_messages.lock() = commit.messages;
        Ok(())
    }
}

struct MockConversationManager {
    retrieval: Arc<MockRetrieval>,
    commit_service: Arc<MockCommitService>,
}

impl brain_services::conversation::ConversationManager for MockConversationManager {
    fn ingest_interaction(
        &self,
        session_id: &SessionId,
        prompt: &str,
        response: &str,
        _policy: brain_services::conversation::IngestionPolicy,
    ) -> Result<(), BrainError> {
        let user_msg = brain_domain::Message::new(
            brain_domain::MessageId::new(),
            brain_domain::MessageRole::User,
            prompt.to_string(),
        );
        let assistant_msg = brain_domain::Message::new(
            brain_domain::MessageId::new(),
            brain_domain::MessageRole::Assistant,
            response.to_string(),
        );
        let commit = MemoryCommit::new(vec![], vec![], vec![user_msg, assistant_msg]);
        self.commit_service.commit(session_id, commit)
    }

    fn build_context_window(
        &self,
        session_id: &SessionId,
        _budget: brain_services::conversation::ContextBudget,
    ) -> Result<brain_services::conversation::ContextWindow, BrainError> {
        let memories = self.retrieval.retrieve(session_id, "", 10)?;
        Ok(brain_services::conversation::ContextWindow::new(
            vec![],
            None,
            memories,
        ))
    }

    fn promote_memories(&self, _session_id: &SessionId) -> Result<(), BrainError> {
        Ok(())
    }

    fn summarize_conversation(
        &self,
        _session_id: &SessionId,
    ) -> Result<brain_services::conversation::ConversationSummary, BrainError> {
        Err(BrainError::Validation {
            message: "unsupported".to_string(),
        })
    }

    fn create_checkpoint(
        &self,
        _session_id: &SessionId,
        _label: &str,
    ) -> Result<ConversationId, BrainError> {
        Ok(ConversationId::new())
    }

    fn restore_checkpoint(
        &self,
        _session_id: &SessionId,
        _checkpoint_id: &ConversationId,
    ) -> Result<(), BrainError> {
        Ok(())
    }

    fn prune_memories(&self, _session_id: &SessionId) -> Result<usize, BrainError> {
        Ok(0)
    }

    fn archive_conversation(&self, _session_id: &SessionId) -> Result<(), BrainError> {
        Ok(())
    }
}

// --- Test Cases ---

#[tokio::test]
async fn test_successful_execution_with_zero_tool_calls() {
    let planner = Arc::new(MockPlanner {
        tool_calls: vec![],
        should_fail: false,
    });
    let chat = Arc::new(MockChat {
        response: "Hello, here is my response".to_string(),
        should_fail: false,
    });
    let retrieval = Arc::new(MockRetrieval { nodes: vec![] });
    let tool_executor = Arc::new(MockToolExecutor {
        should_fail: false,
        latency: Duration::ZERO,
        execution_count: Arc::new(AtomicUsize::new(0)),
    });
    let committed = Arc::new(AtomicBool::new(false));
    let committed_nodes = Arc::new(parking_lot::Mutex::new(vec![]));
    let committed_messages = Arc::new(parking_lot::Mutex::new(vec![]));
    let commit_service = Arc::new(MockCommitService {
        committed: committed.clone(),
        should_fail: false,
        committed_nodes: committed_nodes.clone(),
        committed_messages: committed_messages.clone(),
    });
    let conversation_manager = Arc::new(MockConversationManager {
        retrieval,
        commit_service,
    });

    let streaming_runtime = Arc::new(StreamingRuntime::new(Arc::new(DefaultStreamEventMapper)));
    let engine = create_default_test_engine(
        ExecutionPolicy::default(),
        conversation_manager,
        tool_executor,
        planner,
        chat,
        streaming_runtime,
    );

    let session_id = SessionId::new();
    let conv_id = ConversationId::new();
    let handle = engine.execute(session_id, conv_id, "hello");

    // Wait for the final result
    let result = handle.wait().await.unwrap();

    assert_eq!(result.response_text(), "Hello, here is my response");
    assert!(committed.load(Ordering::SeqCst));
    assert_eq!(committed_messages.lock().len(), 2); // User + Assistant messages
    assert_eq!(committed_messages.lock()[0].content, "hello");
    assert_eq!(
        committed_messages.lock()[1].content,
        "Hello, here is my response"
    );
    assert_eq!(result.metrics().step_count, 0); // zero tools executed
}

#[tokio::test]
async fn test_planner_invalid_tool_failure() {
    let tool_call = ToolCall::new(
        "call-1".to_string(),
        "failing_tool".to_string(),
        HashMap::new(),
    );
    let planner = Arc::new(MockPlanner {
        tool_calls: vec![tool_call],
        should_fail: false,
    });
    let chat = Arc::new(MockChat {
        response: "Hello".to_string(),
        should_fail: false,
    });
    let retrieval = Arc::new(MockRetrieval { nodes: vec![] });
    let tool_executor = Arc::new(MockToolExecutor {
        should_fail: true, // Fail the tool
        latency: Duration::ZERO,
        execution_count: Arc::new(AtomicUsize::new(0)),
    });
    let committed = Arc::new(AtomicBool::new(false));
    let commit_service = Arc::new(MockCommitService {
        committed: committed.clone(),
        should_fail: false,
        committed_nodes: Arc::new(parking_lot::Mutex::new(vec![])),
        committed_messages: Arc::new(parking_lot::Mutex::new(vec![])),
    });
    let conversation_manager = Arc::new(MockConversationManager {
        retrieval,
        commit_service,
    });

    let streaming_runtime = Arc::new(StreamingRuntime::new(Arc::new(DefaultStreamEventMapper)));
    let engine = create_default_test_engine(
        ExecutionPolicy::default(),
        conversation_manager,
        tool_executor,
        planner,
        chat,
        streaming_runtime,
    );

    let session_id = SessionId::new();
    let conv_id = ConversationId::new();
    let handle = engine.execute(session_id, conv_id, "hello");

    let res = handle.wait().await;
    assert!(res.is_err()); // Pipeline failed because tool failed
    assert!(!committed.load(Ordering::SeqCst)); // Reasoning/Commit bypassed
}

#[tokio::test]
async fn test_tool_timeout_and_cancellation() {
    let tool_call = ToolCall::new(
        "call-2".to_string(),
        "slow_tool".to_string(),
        HashMap::new(),
    );
    let planner = Arc::new(MockPlanner {
        tool_calls: vec![tool_call],
        should_fail: false,
    });
    let chat = Arc::new(MockChat {
        response: "Hello".to_string(),
        should_fail: false,
    });
    let retrieval = Arc::new(MockRetrieval { nodes: vec![] });
    // Simulate a slow tool call
    let tool_executor = Arc::new(MockToolExecutor {
        should_fail: false,
        latency: Duration::from_millis(50),
        execution_count: Arc::new(AtomicUsize::new(0)),
    });
    let committed = Arc::new(AtomicBool::new(false));
    let commit_service = Arc::new(MockCommitService {
        committed: committed.clone(),
        should_fail: false,
        committed_nodes: Arc::new(parking_lot::Mutex::new(vec![])),
        committed_messages: Arc::new(parking_lot::Mutex::new(vec![])),
    });
    let conversation_manager = Arc::new(MockConversationManager {
        retrieval,
        commit_service,
    });

    let streaming_runtime = Arc::new(StreamingRuntime::new(Arc::new(DefaultStreamEventMapper)));
    let engine = create_default_test_engine(
        ExecutionPolicy::default(),
        conversation_manager,
        tool_executor,
        planner,
        chat,
        streaming_runtime,
    );

    let session_id = SessionId::new();
    let conv_id = ConversationId::new();
    let handle = engine.execute(session_id, conv_id, "hello");

    // Cancel immediately
    handle.cancel();
    assert_eq!(handle.status(), ExecutionStatus::Cancelled);

    let res = handle.wait().await;
    assert!(res.is_err());
    assert!(!committed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_chat_model_failure() {
    let planner = Arc::new(MockPlanner {
        tool_calls: vec![],
        should_fail: false,
    });
    let chat = Arc::new(MockChat {
        response: "".to_string(),
        should_fail: true, // Chat reasoning fails
    });
    let retrieval = Arc::new(MockRetrieval { nodes: vec![] });
    let tool_executor = Arc::new(MockToolExecutor {
        should_fail: false,
        latency: Duration::ZERO,
        execution_count: Arc::new(AtomicUsize::new(0)),
    });
    let committed = Arc::new(AtomicBool::new(false));
    let commit_service = Arc::new(MockCommitService {
        committed: committed.clone(),
        should_fail: false,
        committed_nodes: Arc::new(parking_lot::Mutex::new(vec![])),
        committed_messages: Arc::new(parking_lot::Mutex::new(vec![])),
    });
    let conversation_manager = Arc::new(MockConversationManager {
        retrieval,
        commit_service,
    });

    let streaming_runtime = Arc::new(StreamingRuntime::new(Arc::new(DefaultStreamEventMapper)));
    let engine = create_default_test_engine(
        ExecutionPolicy::default(),
        conversation_manager,
        tool_executor,
        planner,
        chat,
        streaming_runtime,
    );

    let session_id = SessionId::new();
    let conv_id = ConversationId::new();
    let handle = engine.execute(session_id, conv_id, "hello");

    let res = handle.wait().await;
    assert!(res.is_err());
    assert!(!committed.load(Ordering::SeqCst)); // Commit bypassed
}

#[tokio::test]
async fn test_memory_commit_failure() {
    let planner = Arc::new(MockPlanner {
        tool_calls: vec![],
        should_fail: false,
    });
    let chat = Arc::new(MockChat {
        response: "Hello".to_string(),
        should_fail: false,
    });
    let retrieval = Arc::new(MockRetrieval { nodes: vec![] });
    let tool_executor = Arc::new(MockToolExecutor {
        should_fail: false,
        latency: Duration::ZERO,
        execution_count: Arc::new(AtomicUsize::new(0)),
    });
    let commit_service = Arc::new(MockCommitService {
        committed: Arc::new(AtomicBool::new(false)),
        should_fail: true, // Commit fails
        committed_nodes: Arc::new(parking_lot::Mutex::new(vec![])),
        committed_messages: Arc::new(parking_lot::Mutex::new(vec![])),
    });
    let conversation_manager = Arc::new(MockConversationManager {
        retrieval,
        commit_service,
    });

    let streaming_runtime = Arc::new(StreamingRuntime::new(Arc::new(DefaultStreamEventMapper)));
    let engine = create_default_test_engine(
        ExecutionPolicy::default(),
        conversation_manager,
        tool_executor,
        planner,
        chat,
        streaming_runtime,
    );

    let session_id = SessionId::new();
    let conv_id = ConversationId::new();
    let handle = engine.execute(session_id, conv_id, "hello");

    let res = handle.wait().await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_event_ordering_and_metrics_invariant() {
    let planner = Arc::new(MockPlanner {
        tool_calls: vec![],
        should_fail: false,
    });
    let chat = Arc::new(MockChat {
        response: "Here is a quick answer".to_string(),
        should_fail: false,
    });
    let retrieval = Arc::new(MockRetrieval { nodes: vec![] });
    let tool_executor = Arc::new(MockToolExecutor {
        should_fail: false,
        latency: Duration::ZERO,
        execution_count: Arc::new(AtomicUsize::new(0)),
    });
    let commit_service = Arc::new(MockCommitService {
        committed: Arc::new(AtomicBool::new(false)),
        should_fail: false,
        committed_nodes: Arc::new(parking_lot::Mutex::new(vec![])),
        committed_messages: Arc::new(parking_lot::Mutex::new(vec![])),
    });
    let conversation_manager = Arc::new(MockConversationManager {
        retrieval,
        commit_service,
    });

    let streaming_runtime = Arc::new(StreamingRuntime::new(Arc::new(DefaultStreamEventMapper)));
    let engine = create_default_test_engine(
        ExecutionPolicy::default(),
        conversation_manager,
        tool_executor,
        planner,
        chat,
        streaming_runtime.clone(),
    );

    let session_id = SessionId::new();
    let conv_id = ConversationId::new();
    let handle = engine.execute(session_id, conv_id, "hello");

    let stream = streaming_runtime.subscribe(handle.execution_id()).unwrap();
    let mut events = Vec::new();
    use brain_services::agent::streaming::StreamEventPayload;
    while let Some(evt) = stream.next().await {
        events.push(evt);
    }

    let result = handle.wait().await.unwrap();

    // Verify metrics
    assert!(result.metrics().tokens_used > 0);
    assert_eq!(result.metrics().step_count, 0);

    // Verify monotonic sequence numbering and event structure of raw mapped events
    let raw_mapped_events: Vec<&brain_services::agent::streaming::StreamEvent> = events
        .iter()
        .filter(|e| !matches!(e.payload, StreamEventPayload::Timeline(_)))
        .collect();
    for (i, evt) in raw_mapped_events.iter().enumerate() {
        assert_eq!(evt.sequence, (i + 1) as u64);
    }

    // Assert specific event types occur
    assert!(events.iter().any(|e| matches!(e.payload, StreamEventPayload::Progress { .. })));
    assert!(events.iter().any(|e| matches!(
        e.payload,
        StreamEventPayload::Stage(brain_services::agent::streaming::StageEvent {
            stage: "Planning",
            status: brain_services::agent::streaming::StageStatus::Started,
        })
    )));
    assert!(events.iter().any(|e| matches!(
        e.payload,
        StreamEventPayload::Stage(brain_services::agent::streaming::StageEvent {
            stage: "Retrieval",
            status: brain_services::agent::streaming::StageStatus::Completed,
        })
    )));
    // TokenStreamed occurs during reasoning
    let token_events: Vec<&brain_services::agent::streaming::StreamEvent> = events
        .iter()
        .filter(|e| matches!(e.payload, StreamEventPayload::Token { .. }))
        .collect();
    assert!(!token_events.is_empty());
}

#[tokio::test]
async fn test_idempotent_repeated_cancellation() {
    let planner = Arc::new(MockPlanner {
        tool_calls: vec![],
        should_fail: false,
    });
    let chat = Arc::new(MockChat {
        response: "Hello".to_string(),
        should_fail: false,
    });
    let retrieval = Arc::new(MockRetrieval { nodes: vec![] });
    let tool_executor = Arc::new(MockToolExecutor {
        should_fail: false,
        latency: Duration::ZERO,
        execution_count: Arc::new(AtomicUsize::new(0)),
    });
    let commit_service = Arc::new(MockCommitService {
        committed: Arc::new(AtomicBool::new(false)),
        should_fail: false,
        committed_nodes: Arc::new(parking_lot::Mutex::new(vec![])),
        committed_messages: Arc::new(parking_lot::Mutex::new(vec![])),
    });
    let conversation_manager = Arc::new(MockConversationManager {
        retrieval,
        commit_service,
    });

    let streaming_runtime = Arc::new(StreamingRuntime::new(Arc::new(DefaultStreamEventMapper)));
    let engine = create_default_test_engine(
        ExecutionPolicy::default(),
        conversation_manager,
        tool_executor,
        planner,
        chat,
        streaming_runtime,
    );

    let session_id = SessionId::new();
    let conv_id = ConversationId::new();
    let handle = engine.execute(session_id, conv_id, "hello");

    // Repeated cancels
    handle.cancel();
    handle.cancel();
    handle.cancel();
    assert_eq!(handle.status(), ExecutionStatus::Cancelled);

    let res = handle.wait().await;
    assert!(res.is_err());
}

struct MockMultiCallChat {
    responses: Arc<parking_lot::Mutex<Vec<String>>>,
}

impl brain_core::agents::ChatAgent for MockMultiCallChat {
    fn name(&self) -> &str {
        "MockMultiCallChat"
    }

    fn chat(&self, _session_id: SessionId, _prompt: &str) -> Result<String, BrainError> {
        let mut resp = self.responses.lock();
        if resp.is_empty() {
            Ok("Default fallback".to_string())
        } else {
            Ok(resp.remove(0))
        }
    }
}

#[tokio::test]
async fn test_reflection_self_correction_loop() {
    let planner = Arc::new(MockPlanner {
        tool_calls: vec![],
        should_fail: false,
    });
    // First response contains "TODO", second is clean
    let responses = Arc::new(parking_lot::Mutex::new(vec![
        "This is a TODO placeholder response".to_string(),
        "This is a clean response without placeholders".to_string(),
    ]));
    let chat = Arc::new(MockMultiCallChat { responses: responses.clone() });
    let retrieval = Arc::new(MockRetrieval { nodes: vec![] });
    let tool_executor = Arc::new(MockToolExecutor {
        should_fail: false,
        latency: Duration::ZERO,
        execution_count: Arc::new(AtomicUsize::new(0)),
    });
    let committed = Arc::new(AtomicBool::new(false));
    let committed_nodes = Arc::new(parking_lot::Mutex::new(vec![]));
    let committed_messages = Arc::new(parking_lot::Mutex::new(vec![]));
    let commit_service = Arc::new(MockCommitService {
        committed: committed.clone(),
        should_fail: false,
        committed_nodes: committed_nodes.clone(),
        committed_messages: committed_messages.clone(),
    });
    let conversation_manager = Arc::new(MockConversationManager {
        retrieval,
        commit_service,
    });

    let streaming_runtime = Arc::new(StreamingRuntime::new(Arc::new(DefaultStreamEventMapper)));
    let engine = create_default_test_engine(
        ExecutionPolicy::default(),
        conversation_manager,
        tool_executor,
        planner,
        chat,
        streaming_runtime,
    );

    let session_id = SessionId::new();
    let conv_id = ConversationId::new();
    let handle = engine.execute(session_id, conv_id, "hello");

    let result = handle.wait().await.unwrap();
    assert_eq!(result.response_text(), "This is a clean response without placeholders");
    assert!(committed.load(Ordering::SeqCst));
    // The vector should be empty because both responses were removed/consumed
    assert!(responses.lock().is_empty());
}

#[tokio::test]
async fn test_verification_failure_rollback() {
    let planner = Arc::new(MockPlanner {
        tool_calls: vec![],
        should_fail: false,
    });
    let chat = Arc::new(MockChat {
        response: "This is unsafe".to_string(),
        should_fail: false,
    });
    let retrieval = Arc::new(MockRetrieval { nodes: vec![] });
    let tool_executor = Arc::new(MockToolExecutor {
        should_fail: false,
        latency: Duration::ZERO,
        execution_count: Arc::new(AtomicUsize::new(0)),
    });
    let committed = Arc::new(AtomicBool::new(false));
    let committed_nodes = Arc::new(parking_lot::Mutex::new(vec![]));
    let committed_messages = Arc::new(parking_lot::Mutex::new(vec![]));
    let commit_service = Arc::new(MockCommitService {
        committed: committed.clone(),
        should_fail: false,
        committed_nodes: committed_nodes.clone(),
        committed_messages: committed_messages.clone(),
    });
    let conversation_manager = Arc::new(MockConversationManager {
        retrieval,
        commit_service,
    });

    let streaming_runtime = Arc::new(StreamingRuntime::new(Arc::new(DefaultStreamEventMapper)));
    let engine = create_default_test_engine(
        ExecutionPolicy::default(),
        conversation_manager,
        tool_executor,
        planner,
        chat,
        streaming_runtime,
    );

    let session_id = SessionId::new();
    let conv_id = ConversationId::new();
    let handle = engine.execute(session_id, conv_id, "hello");

    let res = handle.wait().await;
    assert!(res.is_err());
    // Verification failed, so CommitStage was never run, committed remains false!
    assert!(!committed.load(Ordering::SeqCst));
}
