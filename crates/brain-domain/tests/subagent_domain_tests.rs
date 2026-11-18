use brain_domain::{
    DomainEvent, SessionId, Subagent, SubagentId, SubagentStatus, SubagentTransitionError,
};
use std::str::FromStr;

#[test]
fn test_subagent_id_properties() {
    let id1 = SubagentId::new();
    let id2 = id1; // Copy trait
    assert_eq!(id1, id2);

    let id_str = id1.to_string();
    let parsed = SubagentId::from_str(&id_str).expect("parse subagent id");
    assert_eq!(id1, parsed);
}

#[test]
fn test_subagent_happy_lifecycle_and_events() {
    let parent_id = SessionId::new();
    let child_id = SessionId::new();
    let subagent_id = SubagentId::new();

    let mut subagent = Subagent::new(
        subagent_id,
        parent_id,
        child_id,
        "coder",
        "worktree",
        1,
        1000,
    )
    .expect("valid subagent");

    assert_eq!(subagent.status, SubagentStatus::Created);
    assert_eq!(subagent.depth, 1);
    assert!(!subagent.status.is_terminal());

    // Start
    let start_event = subagent.start(1010).expect("start subagent");
    assert_eq!(subagent.status, SubagentStatus::Running);
    assert_eq!(
        start_event,
        DomainEvent::SubagentStarted {
            subagent_id,
            parent_session_id: parent_id,
            subagent_session_id: child_id,
            role: "coder".to_string(),
            started_at: 1010,
        }
    );

    // Progress
    let progress_event = subagent
        .progress("Compiling crates...", 1020)
        .expect("progress");
    assert_eq!(
        progress_event,
        DomainEvent::SubagentProgressed {
            subagent_id,
            progress_message: "Compiling crates...".to_string(),
            timestamp: 1020,
        }
    );

    // Complete
    let complete_event = subagent
        .complete("All tests passed", 1030)
        .expect("complete");
    assert_eq!(subagent.status, SubagentStatus::Completed);
    assert!(subagent.status.is_terminal());
    assert_eq!(
        complete_event,
        DomainEvent::SubagentCompleted {
            subagent_id,
            result: "All tests passed".to_string(),
            completed_at: 1030,
        }
    );

    // Drain events
    let events = subagent.take_events();
    assert_eq!(events.len(), 3);
    assert!(subagent.take_events().is_empty());
}

#[test]
fn test_terminal_state_immutability() {
    let parent_id = SessionId::new();
    let child_id = SessionId::new();
    let subagent_id = SubagentId::new();

    // 1. Completed state is immutable
    let mut subagent = Subagent::new(
        subagent_id,
        parent_id,
        child_id,
        "researcher",
        "task",
        1,
        1000,
    )
    .expect("valid");

    subagent.start(1005).unwrap();
    subagent.complete("Success", 1010).unwrap();
    assert!(subagent.status.is_terminal());

    let err = subagent.start(1015).unwrap_err();
    assert!(matches!(
        err,
        SubagentTransitionError::TerminalStateImmutable { .. }
    ));

    let err2 = subagent.progress("More work", 1020).unwrap_err();
    assert!(matches!(
        err2,
        SubagentTransitionError::TerminalStateImmutable { .. }
    ));

    let err3 = subagent.fail("Belated failure", 1025).unwrap_err();
    assert!(matches!(
        err3,
        SubagentTransitionError::TerminalStateImmutable { .. }
    ));

    let err4 = subagent.cancel("Belated cancel", 1030).unwrap_err();
    assert!(matches!(
        err4,
        SubagentTransitionError::TerminalStateImmutable { .. }
    ));

    // 2. Failed state is immutable
    let mut subagent_failed = Subagent::new(
        SubagentId::new(),
        parent_id,
        child_id,
        "researcher",
        "task",
        1,
        1000,
    )
    .expect("valid");
    subagent_failed.start(1005).unwrap();
    subagent_failed.fail("Network error", 1010).unwrap();
    assert!(subagent_failed.status.is_terminal());

    assert!(matches!(
        subagent_failed.start(1015).unwrap_err(),
        SubagentTransitionError::TerminalStateImmutable { .. }
    ));
    assert!(matches!(
        subagent_failed.complete("Revived", 1020).unwrap_err(),
        SubagentTransitionError::TerminalStateImmutable { .. }
    ));

    // 3. Cancelled state is immutable
    let mut subagent_cancelled = Subagent::new(
        SubagentId::new(),
        parent_id,
        child_id,
        "researcher",
        "task",
        1,
        1000,
    )
    .expect("valid");
    subagent_cancelled.cancel("User aborted", 1005).unwrap();
    assert!(subagent_cancelled.status.is_terminal());

    assert!(matches!(
        subagent_cancelled.start(1010).unwrap_err(),
        SubagentTransitionError::TerminalStateImmutable { .. }
    ));
}

#[test]
fn test_anti_recursion_depth_guardrail() {
    let parent_id = SessionId::new();
    let child_id = SessionId::new();

    // Depths 1, 2, 3 succeed
    for d in 1..=3 {
        assert!(Subagent::new(
            SubagentId::new(),
            parent_id,
            child_id,
            "coder",
            "task",
            d,
            1000,
        )
        .is_ok());
    }

    // Depth 4 fails
    let err = Subagent::new(
        SubagentId::new(),
        parent_id,
        child_id,
        "coder",
        "task",
        4,
        1000,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("exceeds anti-recursion guardrail limit of 3"));
}

#[test]
fn test_subagent_serialization_roundtrip() {
    let parent_id = SessionId::new();
    let child_id = SessionId::new();
    let subagent_id = SubagentId::new();

    let mut subagent = Subagent::new(
        subagent_id,
        parent_id,
        child_id,
        "reviewer",
        "worktree",
        2,
        5000,
    )
    .expect("valid");
    subagent.set_working_dir("/path/to/worktree");
    subagent.start(5010).unwrap();

    let json = serde_json::to_string(&subagent).expect("serialize");
    let restored: Subagent = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(subagent.id, restored.id);
    assert_eq!(subagent.parent_session_id, restored.parent_session_id);
    assert_eq!(subagent.subagent_session_id, restored.subagent_session_id);
    assert_eq!(subagent.role, restored.role);
    assert_eq!(subagent.agent_type, restored.agent_type);
    assert_eq!(subagent.status, restored.status);
    assert_eq!(subagent.depth, restored.depth);
    assert_eq!(subagent.working_dir, restored.working_dir);
}
