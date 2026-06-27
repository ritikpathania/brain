use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use brain_core::errors::BrainError;
use brain_core::services::RetrievalService;
use brain_domain::{Conversation, ConversationId, MemoryDTO, SessionId, ToolCall};
use brain_services::agent::engine::AgentExecutionEngine;
use brain_services::agent::{
    AgentExecutionEvent, AgentExecutionEventPayload, AgentToolExecutor, ExecutionPolicy,
    ExecutionStatus, MemoryCommit, MemoryCommitService,
};

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
        _history: &Conversation,
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

    let engine = AgentExecutionEngine::new(
        ExecutionPolicy::default(),
        retrieval,
        commit_service,
        tool_executor,
        planner,
        chat,
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

    let engine = AgentExecutionEngine::new(
        ExecutionPolicy::default(),
        retrieval,
        commit_service,
        tool_executor,
        planner,
        chat,
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

    let engine = AgentExecutionEngine::new(
        ExecutionPolicy::default(),
        retrieval,
        commit_service,
        tool_executor,
        planner,
        chat,
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

    let engine = AgentExecutionEngine::new(
        ExecutionPolicy::default(),
        retrieval,
        commit_service,
        tool_executor,
        planner,
        chat,
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

    let engine = AgentExecutionEngine::new(
        ExecutionPolicy::default(),
        retrieval,
        commit_service,
        tool_executor,
        planner,
        chat,
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

    let engine = AgentExecutionEngine::new(
        ExecutionPolicy::default(),
        retrieval,
        commit_service,
        tool_executor,
        planner,
        chat,
    );

    let session_id = SessionId::new();
    let conv_id = ConversationId::new();
    let mut handle = engine.execute(session_id, conv_id, "hello");

    let mut events = Vec::new();
    let rx = handle.stream_receiver();
    while let Some(evt) = rx.recv().await {
        events.push(evt);
    }

    let result = handle.wait().await.unwrap();

    // Verify metrics
    assert!(result.metrics().tokens_used > 0);
    assert_eq!(result.metrics().step_count, 0);

    // Verify monotonic sequence numbering and event structure
    for (i, evt) in events.iter().enumerate() {
        assert_eq!(evt.sequence, (i + 1) as u64);
    }

    // Assert specific event types occur in order
    assert!(matches!(
        events[0].payload,
        AgentExecutionEventPayload::ExecutionStarted { .. }
    ));
    assert!(matches!(
        events[1].payload,
        AgentExecutionEventPayload::PlanningStarted { .. }
    ));
    assert!(matches!(
        events[2].payload,
        AgentExecutionEventPayload::RetrievalCompleted { .. }
    ));
    // TokenStreamed occurs during reasoning
    let token_events: Vec<&AgentExecutionEvent> = events
        .iter()
        .filter(|e| matches!(e.payload, AgentExecutionEventPayload::TokenStreamed { .. }))
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

    let engine = AgentExecutionEngine::new(
        ExecutionPolicy::default(),
        retrieval,
        commit_service,
        tool_executor,
        planner,
        chat,
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
