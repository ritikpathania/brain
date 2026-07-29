//! Integration test suite for Supervision Events & Checkpoint Capability Infrastructure (Phase 8 Milestone 8.2).

use brain_services::planning::{
    CheckpointCapability, ExecutionId, ExecutionPlanId, ExecutionSupervisor, SupervisionError,
    SupervisionEventKind,
};
use uuid::Uuid;

#[test]
fn test_supervision_event_stream_logging() {
    let exec_id = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor = ExecutionSupervisor::new(exec_id, plan_id);
    supervisor.pause().unwrap();
    let chkpt = supervisor.create_checkpoint(0).unwrap();
    let _report = supervisor.restore_checkpoint(&chkpt).unwrap();

    let events = supervisor.events();
    assert!(!events.is_empty());

    let kinds: Vec<SupervisionEventKind> = events.iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&SupervisionEventKind::ExecutionPaused));
    assert!(kinds.contains(&SupervisionEventKind::CheckpointCreated));
    assert!(kinds.contains(&SupervisionEventKind::RecoveryStarted));
    assert!(kinds.contains(&SupervisionEventKind::CheckpointRestored));
    assert!(kinds.contains(&SupervisionEventKind::RecoveryCompleted));
}

#[test]
fn test_checkpoint_capability_set_contract() {
    let exec_id = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor = ExecutionSupervisor::new(exec_id, plan_id);
    supervisor.pause().unwrap();
    let chkpt = supervisor.create_checkpoint(1).unwrap();

    assert!(chkpt
        .capabilities
        .has(CheckpointCapability::SupportsStageResume));
    assert!(chkpt
        .capabilities
        .has(CheckpointCapability::SupportsTaskRetry));
    assert!(chkpt
        .capabilities
        .has(CheckpointCapability::SupportsStateReplay));
}

#[test]
fn test_checkpoint_content_hash_integrity_verification() {
    let exec_id = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor = ExecutionSupervisor::new(exec_id, plan_id);
    supervisor.pause().unwrap();
    let mut chkpt = supervisor.create_checkpoint(0).unwrap();

    // Untampered checkpoint passes integrity check
    assert!(chkpt.verify_integrity().is_ok());

    // Tampering with content hash triggers IntegrityFailure
    chkpt.content_hash = "sha256_chkpt:tampered_hash_value".to_string();
    let res = supervisor.restore_checkpoint(&chkpt);
    assert!(res.is_err());

    if let Err(SupervisionError::IntegrityFailure(msg)) = res {
        assert!(msg.contains("Content hash mismatch"));
    } else {
        panic!("Expected SupervisionError::IntegrityFailure");
    }
}
