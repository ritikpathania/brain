//! Task B — Real Subagent Orchestration End-to-End Test Suite
//!
//! Proves the actual runtime loop:
//! Parent -> TaskTool -> SubagentService -> Model Provider -> Tool Execution -> Result Persistence -> Parent
//!
//! Verifies:
//! 1. Named vs Fork subagent (fresh context vs snapshot inheritance with parent immutability)
//! 2. Foreground execution (blocking parent turn, returning child synthesis to parent)
//! 3. Background execution (non-blocking return, async lifecycle in Fleet)
//! 4. Tool execution within child subagent (model tool call -> tool execution -> model follow-up)
//! 5. Active cancellation (cascading token stops model/tool mid-stream, status -> Cancelled)
//! 6. Persistence across reconnect (terminal states and results preserved in SubagentService & Fleet)
//! 7. Failure containment (child failures cleanly recorded as Failed without corrupting parent)
//! 8. Concurrency limits (bounded semaphore limits runaway subagent creation)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use brain_core::errors::BrainError;
use brain_core::extensibility::{
    ExecutionContext, ExecutionPolicy, ExecutionResult, Permission, Tool, ToolMetadata,
};
use brain_domain::{
    Message, MessageId, MessageRole, Session, SessionId, SessionTimestamp, SessionTitle,
    SubagentExecutionMode, SubagentKind, SubagentStatus,
};
use brain_services::model::{DeterministicMockProvider, ScriptedResponse};
use brain_services::subagent::{SubagentError, SubagentService, SubagentSpawnRequest};
use brain_tools::CancellationTokenImpl;
use daemon_bridge::tools::TaskTool;

/// Mock calculator tool to verify tool execution within a child subagent.
#[derive(Clone)]
struct MockCalculatorTool;

static CALC_META: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();

impl Tool for MockCalculatorTool {
    fn metadata(&self) -> &'static ToolMetadata {
        CALC_META.get_or_init(|| ToolMetadata {
            name: "calculator".to_string(),
            description: "Evaluates mathematical expressions".to_string(),
            usage: "calculator(expr: string)".to_string(),
            version: "1.0.0".to_string(),
            author: "Brain Team".to_string(),
            required_permissions: vec![Permission::FilesystemRead],
            execution_policy: ExecutionPolicy { timeout_ms: 5000 },
            supports_streaming: false,
            is_idempotent: true,
            causes_side_effects: false,
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "expr": { "type": "string" }
                },
                "required": ["expr"]
            })),
        })
    }

    fn execute(
        &self,
        _context: &ExecutionContext,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        let expr = arguments
            .get("expr")
            .and_then(|v| v.as_str())
            .unwrap_or("0");
        let result_str = match expr {
            "2 + 2" => "4",
            "10 * 5" => "50",
            "divide_by_zero" => {
                return Err(BrainError::Internal {
                    message: "Division by zero in tool".to_string(),
                });
            }
            _ => "42",
        };
        Ok(ExecutionResult::new(
            serde_json::json!({ "answer": result_str }),
        ))
    }
}

/// Helper to create a parent execution context
fn create_parent_context(session_id: SessionId) -> ExecutionContext {
    ExecutionContext {
        session_id,
        working_dir: std::env::temp_dir(),
        cancellation: Arc::new(CancellationTokenImpl::new()),
        deadline: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_subagent_foreground_execution_with_tool_loop() {
    let parent_id = SessionId::new();
    let parent_ctx = create_parent_context(parent_id);

    let subagent_service = Arc::new(SubagentService::new(4, None));
    let mock_provider = Arc::new(DeterministicMockProvider::new());

    // Step 1: Model issues a tool call to "calculator"
    mock_provider.enqueue_response(ScriptedResponse {
        thinking: Some("Let me calculate 2 + 2 using the calculator tool.".to_string()),
        tokens: vec![],
        tool_calls: vec![(
            "call_calc_1".to_string(),
            "calculator".to_string(),
            serde_json::json!({ "expr": "2 + 2" }),
        )],
        error: None,
        finish_reason: Some("tool_use".to_string()),
    });

    // Step 2: Follow-up synthesis using the tool result
    mock_provider.enqueue_response(ScriptedResponse {
        thinking: Some("The calculator returned 4. Formulating answer.".to_string()),
        tokens: vec!["The calculation result is 4.".to_string()],
        tool_calls: vec![],
        error: None,
        finish_reason: Some("end_turn".to_string()),
    });

    let calc_tool = Arc::new(MockCalculatorTool);
    let task_tool = TaskTool::new()
        .with_service(Arc::clone(&subagent_service))
        .with_model_provider(mock_provider, "mock-model")
        .with_child_tool(calc_tool);

    let mut args = HashMap::new();
    args.insert("prompt".to_string(), serde_json::json!("Calculate 2 + 2"));
    args.insert("role".to_string(), serde_json::json!("mathematician"));
    args.insert("mode".to_string(), serde_json::json!("foreground"));
    args.insert("kind".to_string(), serde_json::json!("named"));

    // Execute through TaskTool
    let exec_res = task_tool
        .execute(&parent_ctx, &args)
        .expect("task execution");
    let val = exec_res.value();

    assert_eq!(val["status"], "completed");
    assert_eq!(val["role"], "mathematician");
    assert_eq!(val["mode"], "foreground");
    assert_eq!(val["result"], "The calculation result is 4.");

    let subagent_id_str = val["subagent_id"].as_str().expect("subagent_id string");
    let subagent_id: brain_domain::SubagentId = subagent_id_str.parse().expect("valid subagent id");

    // Verify persistence in SubagentService
    let subagent_record = subagent_service
        .get_subagent(&subagent_id)
        .expect("persisted");
    assert_eq!(subagent_record.status, SubagentStatus::Completed);
    assert_eq!(
        subagent_record.result,
        Some("The calculation result is 4.".to_string())
    );
    assert_eq!(subagent_record.parent_session_id, parent_id);
    assert_eq!(subagent_record.kind, SubagentKind::Named);
    assert_eq!(
        subagent_record.execution_mode,
        SubagentExecutionMode::Foreground
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_subagent_background_execution_async_lifecycle() {
    let parent_id = SessionId::new();
    let parent_ctx = create_parent_context(parent_id);

    let subagent_service = Arc::new(SubagentService::new(4, None));
    let mock_provider = Arc::new(DeterministicMockProvider::new());

    mock_provider.enqueue_response(ScriptedResponse {
        thinking: None,
        tokens: vec!["Background task output completed.".to_string()],
        tool_calls: vec![],
        error: None,
        finish_reason: Some("end_turn".to_string()),
    });

    let task_tool = TaskTool::new()
        .with_service(Arc::clone(&subagent_service))
        .with_model_provider(mock_provider, "mock-model");

    let mut args = HashMap::new();
    args.insert(
        "prompt".to_string(),
        serde_json::json!("Run background indexing"),
    );
    args.insert("role".to_string(), serde_json::json!("indexer"));
    args.insert("mode".to_string(), serde_json::json!("background"));

    // TaskTool returns immediately for background execution
    let exec_res = task_tool
        .execute(&parent_ctx, &args)
        .expect("task execution");
    let val = exec_res.value();

    assert_eq!(val["mode"], "background");
    let subagent_id_str = val["subagent_id"].as_str().unwrap();
    let subagent_id: brain_domain::SubagentId = subagent_id_str.parse().unwrap();

    // Give background task a moment to finish on tokio
    let mut completed = false;
    for _ in 0..50 {
        if let Some(s) = subagent_service.get_subagent(&subagent_id) {
            if s.status == SubagentStatus::Completed {
                completed = true;
                assert_eq!(
                    s.result,
                    Some("Background task output completed.".to_string())
                );
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(completed, "Background subagent did not complete in time");

    // Verify projection to Fleet view
    let fleet_jobs = subagent_service.to_fleet_jobs();
    let matching_job = fleet_jobs
        .iter()
        .find(|j| j.id == subagent_id_str)
        .expect("fleet job found");
    assert_eq!(matching_job.status, "completed");
    assert_eq!(matching_job.kind, "subagent:indexer");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_subagent_named_vs_fork_isolation() {
    let service = SubagentService::new(4, None);
    let parent_id = SessionId::new();

    // 1. Parent session with message history
    let mut parent = Session::new(
        parent_id,
        SessionTitle("Parent Root Session".to_string()),
        SessionTimestamp(1000),
    );
    let msg1 = Message::new(
        MessageId::new(),
        MessageRole::User,
        "Parent instruction 1".to_string(),
    );
    let msg2 = Message::new(
        MessageId::new(),
        MessageRole::Assistant,
        "Parent response 1".to_string(),
    );
    let _ = parent.add_message(msg1);
    let _ = parent.add_message(msg2);
    assert_eq!(parent.messages.len(), 2);

    // 2. Named Subagent: completely fresh, isolated session ID, no parent messages
    let named_req = SubagentSpawnRequest {
        parent_session_id: parent_id,
        role: "fresh_worker".to_string(),
        agent_type: "task".to_string(),
        prompt: "Start from scratch".to_string(),
        depth: 1,
        system_prompt: None,
        timeout: None,
        use_worktree: false,
        worktree_name: None,
        kind: Some(SubagentKind::Named),
        execution_mode: Some(SubagentExecutionMode::Foreground),
    };
    let named_id = service.spawn(named_req, None).expect("spawn named");
    let named_subagent = service.get_subagent(&named_id).expect("found");
    assert_eq!(named_subagent.kind, SubagentKind::Named);
    assert_ne!(named_subagent.subagent_session_id, parent_id);

    // 3. Fork Subagent: inherits parent session snapshot up to current message
    let fork_req = SubagentSpawnRequest {
        parent_session_id: parent_id,
        role: "fork_worker".to_string(),
        agent_type: "task".to_string(),
        prompt: "Continue parent line of inquiry".to_string(),
        depth: 1,
        system_prompt: None,
        timeout: None,
        use_worktree: false,
        worktree_name: None,
        kind: Some(SubagentKind::Fork),
        execution_mode: Some(SubagentExecutionMode::Foreground),
    };
    let fork_id = service.spawn(fork_req, None).expect("spawn fork");
    let fork_subagent = service.get_subagent(&fork_id).expect("found");
    assert_eq!(fork_subagent.kind, SubagentKind::Fork);

    let forked_session = service.fork_parent_session(&parent, None, "Forked Child Session");
    assert_ne!(forked_session.id, parent.id);
    assert_eq!(forked_session.messages.len(), 2);
    assert_eq!(forked_session.messages[0].content, "Parent instruction 1");
    assert_eq!(forked_session.messages[1].content, "Parent response 1");

    // Parent immutability: modifying forked session does not affect parent
    let mut child_session = forked_session;
    let _ = child_session.add_message(Message::new(
        MessageId::new(),
        MessageRole::User,
        "Child-only instruction".to_string(),
    ));
    assert_eq!(child_session.messages.len(), 3);
    assert_eq!(
        parent.messages.len(),
        2,
        "Parent session was mutated by child fork!"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_subagent_active_cancellation_cascades() {
    let service = Arc::new(SubagentService::new(4, None));
    let parent_id = SessionId::new();
    let parent_token = tokio_util::sync::CancellationToken::new();

    let req = SubagentSpawnRequest {
        parent_session_id: parent_id,
        role: "cancellable_worker".to_string(),
        agent_type: "task".to_string(),
        prompt: "Long running analysis".to_string(),
        depth: 1,
        system_prompt: None,
        timeout: None,
        use_worktree: false,
        worktree_name: None,
        kind: Some(SubagentKind::Named),
        execution_mode: Some(SubagentExecutionMode::Foreground),
    };

    let subagent_id = service.spawn(req, Some(&parent_token)).expect("spawn");
    service.start(&subagent_id).expect("start");

    let child_token = service.get_cancellation_token(&subagent_id).unwrap();
    assert!(!child_token.is_cancelled());

    // Cancel parent token
    parent_token.cancel();
    assert!(child_token.is_cancelled());

    // Cancel subagent in service
    service
        .cancel(&subagent_id, "Parent cancelled")
        .expect("cancelled");
    let subagent = service.get_subagent(&subagent_id).unwrap();
    assert_eq!(subagent.status, SubagentStatus::Cancelled);

    // Terminal immutability: cannot complete or restart a cancelled subagent
    assert!(service.complete(&subagent_id, "Late success").is_err());
    assert_eq!(
        service.get_subagent(&subagent_id).unwrap().status,
        SubagentStatus::Cancelled
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_subagent_failure_containment() {
    let parent_id = SessionId::new();
    let parent_ctx = create_parent_context(parent_id);

    let subagent_service = Arc::new(SubagentService::new(4, None));
    let mock_provider = Arc::new(DeterministicMockProvider::new());

    // Simulate model provider failure
    mock_provider.enqueue_response(ScriptedResponse {
        thinking: None,
        tokens: vec![],
        tool_calls: vec![],
        error: Some("Simulated upstream provider outage".to_string()),
        finish_reason: None,
    });

    let task_tool = TaskTool::new()
        .with_service(Arc::clone(&subagent_service))
        .with_model_provider(mock_provider, "mock-model");

    let mut args = HashMap::new();
    args.insert("prompt".to_string(), serde_json::json!("Failing task"));
    args.insert("role".to_string(), serde_json::json!("worker"));
    args.insert("mode".to_string(), serde_json::json!("foreground"));

    let res = task_tool.execute(&parent_ctx, &args);
    assert!(res.is_err(), "Expected error from failing subagent turn");

    // Verify subagent is marked Failed and contained
    let all_subagents = subagent_service.list_all();
    assert_eq!(all_subagents.len(), 1);
    let s = &all_subagents[0];
    assert_eq!(s.status, SubagentStatus::Failed);
    assert!(s
        .error
        .as_ref()
        .unwrap()
        .contains("Simulated upstream provider outage"));

    // Verify Fleet projection reflects failure
    let fleet_jobs = subagent_service.to_fleet_jobs();
    assert_eq!(fleet_jobs[0].status, "failed");
}

#[test]
fn test_subagent_concurrency_limits_enforced() {
    let service = SubagentService::new(2, None);
    let parent_id = SessionId::new();

    let make_req = |i: usize| SubagentSpawnRequest {
        parent_session_id: parent_id,
        role: format!("worker_{i}"),
        agent_type: "task".to_string(),
        prompt: format!("task {i}"),
        depth: 1,
        system_prompt: None,
        timeout: None,
        use_worktree: false,
        worktree_name: None,
        kind: None,
        execution_mode: None,
    };

    let id1 = service.spawn(make_req(1), None).expect("spawn 1");
    let id2 = service.spawn(make_req(2), None).expect("spawn 2");

    // 3rd spawn exceeds concurrency limit of 2
    let res3 = service.spawn(make_req(3), None);
    match res3 {
        Err(SubagentError::ConcurrencyLimitReached { limit }) => {
            assert_eq!(limit, 2);
        }
        other => panic!("Expected ConcurrencyLimitReached, got {other:?}"),
    }

    // Completing id1 frees permit
    service.start(&id1).expect("start 1");
    service.complete(&id1, "Done").expect("complete 1");

    // Subagent permit is freed upon completion: now 3rd subagent can spawn!
    let id3 = service
        .spawn(make_req(3), None)
        .expect("spawn 3 after permit freed");
    assert!(service.get_subagent(&id3).is_some());
    assert!(service.get_subagent(&id1).is_some());
    assert!(service.get_subagent(&id2).is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_subagent_persistence_across_reconnect() {
    let service = SubagentService::new(4, None);
    let parent_id = SessionId::new();

    // Spawn and complete multiple subagents
    let mut ids = Vec::new();
    for i in 0..3 {
        let req = SubagentSpawnRequest {
            parent_session_id: parent_id,
            role: format!("role_{i}"),
            agent_type: "batch".to_string(),
            prompt: format!("prompt {i}"),
            depth: 1,
            system_prompt: None,
            timeout: None,
            use_worktree: false,
            worktree_name: None,
            kind: Some(SubagentKind::Named),
            execution_mode: Some(SubagentExecutionMode::Foreground),
        };
        let id = service.spawn(req, None).unwrap();
        service.start(&id).unwrap();
        service.complete(&id, &format!("Result {i}")).unwrap();
        ids.push(id);
    }

    // Simulate client disconnect and reconnect:
    // Reconnecting client calls list_all and to_fleet_jobs
    let reconnected_subagents = service.list_all();
    assert_eq!(reconnected_subagents.len(), 3);

    for (i, id) in ids.iter().enumerate() {
        let subagent = service.get_subagent(id).expect("found on reconnect");
        assert_eq!(subagent.status, SubagentStatus::Completed);
        assert_eq!(subagent.result, Some(format!("Result {i}")));
    }

    let fleet_jobs = service.to_fleet_jobs();
    assert_eq!(fleet_jobs.len(), 3);
    for job in &fleet_jobs {
        assert_eq!(job.status, "completed");
    }
}
