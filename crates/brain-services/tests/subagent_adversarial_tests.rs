//! Phase 4 Adversarial & Invariant Verification Test Suite (R9)
//!
//! Tests:
//! 1. Concurrent subagent creation and cancellation.
//! 2. Parent termination while children active (cascade cancellation).
//! 3. Terminal state immutability under concurrency.
//! 4. Dirty worktree preservation on cleanup (uncommitted & untracked files).
//! 5. Context isolation (child mutations do not leak into parent).

use brain_domain::{
    Goal, GoalId, Message, MessageId, MessageRole, Session, SessionId, SessionTimestamp,
    SessionTitle, SubagentId, SubagentStatus, SubagentTransitionError,
};
use brain_services::subagent::{SubagentError, SubagentService, SubagentSpawnRequest};
use brain_services::worktree::{WorktreeError, WorktreeService};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::thread;

/// Helper to initialize an isolated git repository fixture in target/tmp/
fn create_test_repo_fixture(fixture_name: &str) -> (PathBuf, PathBuf) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("tmp")
        .join(format!(
            "worktree_adv_{}_{}",
            fixture_name,
            std::process::id()
        ));

    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("create fixture dir");

    let repo_dir = base.join("repo");
    fs::create_dir_all(&repo_dir).expect("create repo dir");

    Command::new("git")
        .arg("init")
        .arg(&repo_dir)
        .output()
        .expect("git init");

    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["config", "user.name", "Adversarial Runner"])
        .output()
        .expect("git config user.name");
    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["config", "user.email", "adv@example.com"])
        .output()
        .expect("git config user.email");

    let readme = repo_dir.join("README.md");
    let mut f = File::create(&readme).expect("create readme");
    writeln!(f, "# Adversarial Root").expect("write readme");

    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["add", "README.md"])
        .output()
        .expect("git add");

    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["commit", "-m", "Root commit"])
        .output()
        .expect("git commit");

    let worktrees_dir = base.join("worktrees");
    fs::create_dir_all(&worktrees_dir).expect("create worktrees dir");

    (repo_dir, worktrees_dir)
}

#[test]
fn test_concurrent_subagent_creation_and_cancellation() {
    let service = Arc::new(SubagentService::new(8, None));
    let parent_id = SessionId::new();

    let mut handles = Vec::new();
    for i in 0..8 {
        let svc = Arc::clone(&service);
        handles.push(thread::spawn(move || {
            let req = SubagentSpawnRequest {
                parent_session_id: parent_id,
                role: format!("worker_{i}"),
                agent_type: "task".to_string(),
                prompt: format!("Execute task {i}"),
                depth: 1,
                system_prompt: None,
                timeout: None,
                use_worktree: false,
                worktree_name: None,
                kind: None,
                execution_mode: None,
            };
            let id = svc.spawn(req, None).expect("concurrent spawn");
            svc.start(&id).expect("start");

            if i % 2 == 0 {
                svc.complete(&id, "Done").expect("complete");
            } else {
                svc.cancel(&id, "Aborted").expect("cancel");
            }
            id
        }));
    }

    let subagent_ids: Vec<SubagentId> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(subagent_ids.len(), 8);

    for (i, id) in subagent_ids.iter().enumerate() {
        let s = service.get_subagent(id).expect("registered");
        assert!(s.status.is_terminal());
        if i % 2 == 0 {
            assert_eq!(s.status, SubagentStatus::Completed);
        } else {
            assert_eq!(s.status, SubagentStatus::Cancelled);
        }
    }
}

#[test]
fn test_parent_termination_while_children_active() {
    let service = SubagentService::new(8, None);
    let parent_id = SessionId::new();
    let independent_parent_id = SessionId::new();

    let make_req = |parent: SessionId, role: &str| SubagentSpawnRequest {
        parent_session_id: parent,
        role: role.to_string(),
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

    let child1 = service.spawn(make_req(parent_id, "c1"), None).unwrap();
    let child2 = service.spawn(make_req(parent_id, "c2"), None).unwrap();
    let child3 = service.spawn(make_req(parent_id, "c3"), None).unwrap();
    let independent_child = service
        .spawn(make_req(independent_parent_id, "indep"), None)
        .unwrap();

    service.start(&child1).unwrap();
    service.start(&child2).unwrap();
    // child3 completed before parent termination
    service.start(&child3).unwrap();
    service.complete(&child3, "Already done").unwrap();

    // Parent is abruptly terminated
    let cancelled = service.cancel_children_of(&parent_id, "Parent terminated");
    // Only active children (child1 and child2) are cancelled; completed child3 is untouched!
    assert_eq!(cancelled, 2);

    assert_eq!(
        service.get_subagent(&child1).unwrap().status,
        SubagentStatus::Cancelled
    );
    assert_eq!(
        service.get_subagent(&child2).unwrap().status,
        SubagentStatus::Cancelled
    );
    // Already completed child remains Completed (terminal immutability)
    assert_eq!(
        service.get_subagent(&child3).unwrap().status,
        SubagentStatus::Completed
    );

    // Independent parent's child remains uncancelled
    assert_eq!(
        service.get_subagent(&independent_child).unwrap().status,
        SubagentStatus::Created
    );
}

#[test]
fn test_terminal_state_immutability_adversarial() {
    let service = SubagentService::new(4, None);
    let parent_id = SessionId::new();

    let id = service
        .spawn(
            SubagentSpawnRequest {
                parent_session_id: parent_id,
                role: "worker".to_string(),
                agent_type: "task".to_string(),
                prompt: "task".to_string(),
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
        .unwrap();

    service.start(&id).unwrap();
    service.complete(&id, "Finished").unwrap();

    // Attempt to start again
    let err1 = service.start(&id).unwrap_err();
    assert!(matches!(
        err1,
        SubagentError::TransitionError(SubagentTransitionError::TerminalStateImmutable { .. })
    ));

    // Attempt to progress again
    let err2 = service.progress(&id, "More work").unwrap_err();
    assert!(matches!(
        err2,
        SubagentError::TransitionError(SubagentTransitionError::TerminalStateImmutable { .. })
    ));

    // Attempt to fail after complete
    let err3 = service.fail(&id, "Belated error").unwrap_err();
    assert!(matches!(
        err3,
        SubagentError::TransitionError(SubagentTransitionError::TerminalStateImmutable { .. })
    ));

    // Attempt to cancel after complete
    let err4 = service.cancel(&id, "Belated cancel").unwrap_err();
    assert!(matches!(
        err4,
        SubagentError::TransitionError(SubagentTransitionError::TerminalStateImmutable { .. })
    ));

    // Terminal status is completely unchanged
    let final_state = service.get_subagent(&id).unwrap();
    assert_eq!(final_state.status, SubagentStatus::Completed);
    assert_eq!(final_state.result, Some("Finished".to_string()));
}

#[test]
fn test_dirty_worktree_preservation_on_cleanup_adversarial() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("dirty_adv");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    let desc = service
        .create_worktree("adv-worktree", None)
        .expect("create");

    // Add multiple dirty files (both untracked and modified tracked)
    let dirty_tracked = desc.path.join("README.md");
    let mut f = File::create(&dirty_tracked).expect("overwrite readme");
    writeln!(f, "# Tampered in worktree").unwrap();

    let untracked_file = desc.path.join("uncommitted_artifact.rs");
    let mut f2 = File::create(&untracked_file).expect("create untracked");
    writeln!(f2, "pub fn valuable_code() -> bool {{ true }}").unwrap();

    // 10 concurrent removal attempts MUST ALL FAIL
    let svc = Arc::new(service);
    let mut handles = Vec::new();
    for _ in 0..10 {
        let s = Arc::clone(&svc);
        handles.push(thread::spawn(move || {
            s.remove_worktree("adv-worktree", false)
        }));
    }

    for h in handles {
        let res = h.join().unwrap();
        assert!(res.is_err());
        match res.unwrap_err() {
            WorktreeError::WorktreeDirty {
                porcelain_status, ..
            } => {
                assert!(
                    porcelain_status.contains("README.md")
                        || porcelain_status.contains("uncommitted_artifact.rs")
                );
            }
            other => panic!("Expected WorktreeDirty, got {other:?}"),
        }
    }

    // CRITICAL: Disk state must be 100% intact
    assert!(desc.path.exists());
    assert!(dirty_tracked.exists());
    assert!(untracked_file.exists());
    let content = fs::read_to_string(&untracked_file).unwrap();
    assert!(content.contains("valuable_code"));
}

#[test]
fn test_context_isolation_child_mutation_does_not_leak_into_parent() {
    let service = SubagentService::new(4, None);
    let parent_id = SessionId::new();
    let mut parent = Session::new(
        parent_id,
        SessionTitle("Parent".to_string()),
        SessionTimestamp(100),
    );

    // Seed parent with 3 messages
    for i in 1..=3 {
        parent.messages.push(Message::new(
            MessageId::new(),
            MessageRole::User,
            format!("Parent message {i}"),
        ));
    }
    // Seed parent with 1 goal
    let goal_id = GoalId::new();
    let _ = parent.add_goal(Goal {
        id: goal_id,
        text: "Complete task".to_string(),
    });

    // Snapshot parent state before fork
    let parent_msgs_len_before = parent.messages.len();
    let parent_goals_len_before = parent.goals.len();
    let parent_msg_ids: Vec<MessageId> = parent.messages.iter().map(|m| m.id).collect();

    // Fork child
    let mut child = service.fork_parent_session(&parent, None, "Child Session");

    // Heavily mutate child session:
    // 1. Append 50 messages
    for i in 1..=50 {
        child.messages.push(Message::new(
            MessageId::new(),
            MessageRole::Assistant,
            format!("Child delta {i}"),
        ));
    }
    // 2. Remove/clear goals on child
    child.goals.clear();
    // 3. Clear some child messages
    child.messages.drain(0..2);

    // Assert parent session is 100% UNTOUCHED
    assert_eq!(parent.messages.len(), parent_msgs_len_before);
    assert_eq!(parent.goals.len(), parent_goals_len_before);
    let parent_msg_ids_after: Vec<MessageId> = parent.messages.iter().map(|m| m.id).collect();
    assert_eq!(parent_msg_ids, parent_msg_ids_after);
}
