//! Integration test suite for WorkerSessionManager, DispatchLifecycleEvent, and pure RecoveryPolicy evaluations (Phase 9 Milestone 9.3).

use brain_services::planning::{
    DispatchLifecycleEvent, DispatchLifecycleEventId, DispatchLifecycleEventKind,
    HeartbeatGracePolicy, ImmediateReassignPolicy, LeaseId, ProtocolNegotiation, RecoveryAction,
    RecoveryContext, RecoveryPolicy, SessionState, TaskAssignment, TaskId, WorkerId,
    WorkerSessionManager,
};
use uuid::Uuid;

#[test]
fn test_worker_session_lifecycle_and_lease_bindings() {
    let mut manager = WorkerSessionManager::new();
    let worker_id = WorkerId(Uuid::new_v4());
    let negotiation = ProtocolNegotiation::default();

    // 1. Create session (Negotiating)
    let session = manager.create_session(worker_id, negotiation, 10000);
    assert_eq!(session.state, SessionState::Negotiating);
    assert_eq!(session.worker_id, worker_id);

    // 2. Activate session (Active)
    manager.activate_session(session.session_id).unwrap();
    let active_session = manager.get_session(session.session_id).unwrap();
    assert_eq!(active_session.state, SessionState::Active);

    // 3. Bind lease
    let lease_id = LeaseId(Uuid::new_v4());
    manager.bind_lease(session.session_id, lease_id).unwrap();
    assert_eq!(
        manager
            .get_session(session.session_id)
            .unwrap()
            .active_leases
            .len(),
        1
    );

    // 4. Close session -> returns bound active leases for invalidation
    let bound = manager.close_session(session.session_id).unwrap();
    assert_eq!(bound.len(), 1);
    assert_eq!(bound[0], lease_id);
    assert_eq!(
        manager.get_session(session.session_id).unwrap().state,
        SessionState::Closed
    );
}

#[test]
fn test_dispatch_lifecycle_events_progress_tracking() {
    let lease_id = LeaseId(Uuid::new_v4());
    let task_id = TaskId(Uuid::new_v4());

    let event_start = DispatchLifecycleEvent {
        event_id: DispatchLifecycleEventId(Uuid::new_v4()),
        lease_id,
        task_id,
        kind: DispatchLifecycleEventKind::TaskStepStarted,
        progress_percent: None,
        timestamp_ms: 10000,
    };

    let event_progress = DispatchLifecycleEvent {
        event_id: DispatchLifecycleEventId(Uuid::new_v4()),
        lease_id,
        task_id,
        kind: DispatchLifecycleEventKind::TaskStepProgress,
        progress_percent: Some(50.0),
        timestamp_ms: 10500,
    };

    let event_completed = DispatchLifecycleEvent {
        event_id: DispatchLifecycleEventId(Uuid::new_v4()),
        lease_id,
        task_id,
        kind: DispatchLifecycleEventKind::TaskStepCompleted,
        progress_percent: Some(100.0),
        timestamp_ms: 11000,
    };

    let events = [event_start, event_progress, event_completed];
    assert_eq!(events.len(), 3);
    assert_eq!(events[1].progress_percent, Some(50.0));
}

#[test]
fn test_pure_recovery_policy_evaluations() {
    let worker_id = WorkerId(Uuid::new_v4());
    let task_id = TaskId(Uuid::new_v4());
    let assignment = TaskAssignment {
        assignment_id: Uuid::new_v4(),
        task_id,
        worker_id,
        assigned_at_ms: 10000,
    };

    let ctx = RecoveryContext {
        assignment: &assignment,
        lease: None,
        last_heartbeat_ms: Some(10000),
        now_ms: 12000, // 2000ms after heartbeat
    };

    // ImmediateReassignPolicy returns ImmediateReassign
    let imm_policy = ImmediateReassignPolicy;
    assert_eq!(
        imm_policy.determine_action(&ctx),
        RecoveryAction::ImmediateReassign
    );

    // HeartbeatGracePolicy (5000ms grace period) returns WaitForHeartbeat since 2000ms <= 5000ms
    let grace_policy = HeartbeatGracePolicy {
        grace_period_ms: 5000,
    };
    assert_eq!(
        grace_policy.determine_action(&ctx),
        RecoveryAction::WaitForHeartbeat
    );

    // Context at 18000ms (8000ms after heartbeat > 5000ms grace) returns ImmediateReassign
    let expired_ctx = RecoveryContext {
        assignment: &assignment,
        lease: None,
        last_heartbeat_ms: Some(10000),
        now_ms: 18000,
    };
    assert_eq!(
        grace_policy.determine_action(&expired_ctx),
        RecoveryAction::ImmediateReassign
    );
}
