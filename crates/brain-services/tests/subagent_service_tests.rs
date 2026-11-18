use brain_domain::{
    Message, MessageId, MessageRole, Session, SessionId, SessionTimestamp, SessionTitle,
    SubagentStatus,
};
use brain_services::subagent::{SubagentService, SubagentSpawnRequest};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[test]
fn test_subagent_spawn_and_lifecycle_transitions() {
    let service = SubagentService::new(4, None);
    let parent_session_id = SessionId::new();

    let req = SubagentSpawnRequest {
        parent_session_id,
        role: "researcher".to_string(),
        agent_type: "general".to_string(),
        prompt: "Analyze codebase architecture".to_string(),
        depth: 1,
        system_prompt: None,
        timeout: Some(Duration::from_secs(30)),
        use_worktree: false,
        worktree_name: None,
        kind: None,
        execution_mode: None,
    };

    let subagent_id = service.spawn(req, None).expect("spawn subagent");

    let subagent = service.get_subagent(&subagent_id).expect("found");
    assert_eq!(subagent.status, SubagentStatus::Created);
    assert_eq!(subagent.parent_session_id, parent_session_id);
    assert_eq!(subagent.role, "researcher");

    // Start
    service.start(&subagent_id).expect("start");
    let started = service.get_subagent(&subagent_id).unwrap();
    assert_eq!(started.status, SubagentStatus::Running);

    // Progress
    service
        .progress(&subagent_id, "Scanned 50 modules")
        .expect("progress");

    // Complete
    service
        .complete(&subagent_id, "Found modular DDD architecture")
        .expect("complete");
    let completed = service.get_subagent(&subagent_id).unwrap();
    assert_eq!(completed.status, SubagentStatus::Completed);
    assert_eq!(
        completed.result,
        Some("Found modular DDD architecture".to_string())
    );
}

#[test]
fn test_context_fork_isolation_and_source_immutability() {
    let service = SubagentService::new(4, None);

    // Create a parent session with 2 messages
    let parent_id = SessionId::new();
    let mut parent = Session::new(
        parent_id,
        SessionTitle("Parent Session".to_string()),
        SessionTimestamp(100),
    );

    let msg1 = Message::new(
        MessageId::new(),
        MessageRole::User,
        "Please refactor the compiler".to_string(),
    );
    let msg2 = Message::new(
        MessageId::new(),
        MessageRole::Assistant,
        "Starting refactor analysis".to_string(),
    );

    parent.messages.push(msg1.clone());
    parent.messages.push(msg2.clone());

    // Fork for subagent
    let child_session = service.fork_parent_session(&parent, Some(&msg1.id), "Subagent Work");

    // INVARIANT 1: Child has messages up to msg1
    assert_eq!(child_session.messages.len(), 1);
    assert_eq!(child_session.messages[0].id, msg1.id);
    assert_ne!(child_session.id, parent.id);

    // INVARIANT 2: Mutating child does NOT affect parent
    let mut child = child_session;
    child.messages.push(Message::new(
        MessageId::new(),
        MessageRole::Assistant,
        "Subagent internal message".to_string(),
    ));

    assert_eq!(parent.messages.len(), 2);
    assert_eq!(parent.messages[1].id, msg2.id);
    assert_eq!(child.messages.len(), 2);
}

#[test]
fn test_bounded_concurrency_semaphore() {
    // Limit to 2 concurrent subagents
    let service = SubagentService::new(2, None);
    let parent_id = SessionId::new();

    let spawn_one = || {
        service.spawn(
            SubagentSpawnRequest {
                parent_session_id: parent_id,
                role: "worker".to_string(),
                agent_type: "task".to_string(),
                prompt: "work".to_string(),
                depth: 1,
                system_prompt: None,
                timeout: None,
                use_worktree: false,
                worktree_name: None,
                kind: None,
                execution_mode: None,
            },
            None,
        )
    };

    let id1 = spawn_one().expect("spawn 1");
    let id2 = spawn_one().expect("spawn 2");

    // Both are present
    assert!(service.get_subagent(&id1).is_some());
    assert!(service.get_subagent(&id2).is_some());
}

#[test]
fn test_cancellation_token_cascading_from_parent() {
    let service = SubagentService::new(4, None);
    let parent_id = SessionId::new();
    let parent_token = CancellationToken::new();

    let req = SubagentSpawnRequest {
        parent_session_id: parent_id,
        role: "worker".to_string(),
        agent_type: "task".to_string(),
        prompt: "work".to_string(),
        depth: 1,
        system_prompt: None,
        timeout: None,
        use_worktree: false,
        worktree_name: None,
        kind: None,
        execution_mode: None,
    };

    let subagent_id = service.spawn(req, Some(&parent_token)).expect("spawn");
    let child_token = service
        .get_cancellation_token(&subagent_id)
        .expect("child token");

    assert!(!child_token.is_cancelled());

    // Cancelling the parent token MUST cascade to child
    parent_token.cancel();
    assert!(child_token.is_cancelled());
}

#[test]
fn test_cancel_children_of_parent_session() {
    let service = SubagentService::new(4, None);
    let parent_id = SessionId::new();
    let other_parent_id = SessionId::new();

    let req1 = SubagentSpawnRequest {
        parent_session_id: parent_id,
        role: "worker1".to_string(),
        agent_type: "task".to_string(),
        prompt: "work1".to_string(),
        depth: 1,
        system_prompt: None,
        timeout: None,
        use_worktree: false,
        worktree_name: None,
        kind: None,
        execution_mode: None,
    };
    let req2 = SubagentSpawnRequest {
        parent_session_id: parent_id,
        role: "worker2".to_string(),
        agent_type: "task".to_string(),
        prompt: "work2".to_string(),
        depth: 1,
        system_prompt: None,
        timeout: None,
        use_worktree: false,
        worktree_name: None,
        kind: None,
        execution_mode: None,
    };
    let req3 = SubagentSpawnRequest {
        parent_session_id: other_parent_id,
        role: "other_worker".to_string(),
        agent_type: "task".to_string(),
        prompt: "other work".to_string(),
        depth: 1,
        system_prompt: None,
        timeout: None,
        use_worktree: false,
        worktree_name: None,
        kind: None,
        execution_mode: None,
    };

    let id1 = service.spawn(req1, None).unwrap();
    let id2 = service.spawn(req2, None).unwrap();
    let id3 = service.spawn(req3, None).unwrap();

    let cancelled_count = service.cancel_children_of(&parent_id, "Parent turn interrupted");
    assert_eq!(cancelled_count, 2);

    assert_eq!(
        service.get_subagent(&id1).unwrap().status,
        SubagentStatus::Cancelled
    );
    assert_eq!(
        service.get_subagent(&id2).unwrap().status,
        SubagentStatus::Cancelled
    );

    // Other parent's child is UNTOUCHED
    assert_eq!(
        service.get_subagent(&id3).unwrap().status,
        SubagentStatus::Created
    );
}

#[test]
fn test_fleet_job_summary_projection() {
    let service = SubagentService::new(4, None);
    let parent_id = SessionId::new();

    let req = SubagentSpawnRequest {
        parent_session_id: parent_id,
        role: "reviewer".to_string(),
        agent_type: "code_audit".to_string(),
        prompt: "Review PR #42 security patches".to_string(),
        depth: 1,
        system_prompt: None,
        timeout: None,
        use_worktree: false,
        worktree_name: None,
        kind: None,
        execution_mode: None,
    };

    let subagent_id = service.spawn(req, None).unwrap();
    service.start(&subagent_id).unwrap();

    let fleet_jobs = service.to_fleet_jobs();
    assert_eq!(fleet_jobs.len(), 1);

    let job = &fleet_jobs[0];
    assert_eq!(job.id, subagent_id.to_string());
    assert_eq!(job.kind, "subagent:reviewer");
    assert_eq!(job.status, "running");
    assert_eq!(job.group, Some("code_audit".to_string()));
    assert!(job.subtext.contains("Review PR #42"));
}
