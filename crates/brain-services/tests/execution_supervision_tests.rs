//! Integration test suite for Execution Supervision, Checkpointing & Recovery Engine (Phase 8 Milestone 8.1).

use brain_services::planning::{
    ExecutionId, ExecutionPlanId, ExecutionSupervisor, SupervisionError, SupervisionState,
};
use uuid::Uuid;

#[test]
fn test_supervisor_state_transitions_happy_path() {
    let exec_id = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor = ExecutionSupervisor::new(exec_id, plan_id);
    assert_eq!(*supervisor.state(), SupervisionState::Active);

    // Active -> Paused
    supervisor.pause().unwrap();
    assert_eq!(*supervisor.state(), SupervisionState::Paused);

    // Paused -> Checkpointed
    let chkpt = supervisor.create_checkpoint(0).unwrap();
    assert_eq!(*supervisor.state(), SupervisionState::Checkpointed);
    assert_eq!(chkpt.schema_version, 1);
    assert_eq!(chkpt.completed_stage_index, 0);

    // Checkpointed -> Recovering -> Active
    let report = supervisor.restore_checkpoint(&chkpt).unwrap();
    assert_eq!(*supervisor.state(), SupervisionState::Active);
    assert_eq!(report.recovered_stage_index, 0);
    assert_eq!(report.skipped_stages_count, 1);
}

#[test]
fn test_supervisor_illegal_state_transition_fails() {
    let exec_id = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor = ExecutionSupervisor::new(exec_id, plan_id);

    // Active -> Resume (illegal, must be Paused first)
    let res = supervisor.resume();
    assert!(res.is_err());
    if let Err(SupervisionError::InvalidStateTransition { from, to }) = res {
        assert_eq!(from, "Active");
        assert_eq!(to, "Active");
    } else {
        panic!("Expected SupervisionError::InvalidStateTransition");
    }
}

#[test]
fn test_supervisor_invalid_checkpoint_mismatch_fails() {
    let exec_id_1 = ExecutionId(Uuid::new_v4());
    let exec_id_2 = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor_1 = ExecutionSupervisor::new(exec_id_1, plan_id);
    let mut supervisor_2 = ExecutionSupervisor::new(exec_id_2, plan_id);

    supervisor_1.pause().unwrap();
    let chkpt = supervisor_1.create_checkpoint(0).unwrap();

    // Attempting to restore supervisor_1's checkpoint into supervisor_2 (ExecutionId mismatch)
    let res = supervisor_2.restore_checkpoint(&chkpt);
    assert!(res.is_err());
    if let Err(SupervisionError::CheckpointMismatch(msg)) = res {
        assert!(msg.contains("ExecutionId mismatch"));
    } else {
        panic!("Expected SupervisionError::CheckpointMismatch");
    }
}

#[test]
fn test_supervisor_recovery_idempotence() {
    let exec_id = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor = ExecutionSupervisor::new(exec_id, plan_id);

    supervisor.pause().unwrap();
    let chkpt = supervisor.create_checkpoint(1).unwrap();

    let report_1 = supervisor.restore_checkpoint(&chkpt).unwrap();
    supervisor.pause().unwrap();
    let _chkpt2 = supervisor.create_checkpoint(1).unwrap();
    let report_2 = supervisor.restore_checkpoint(&chkpt).unwrap();

    // Verify recovery idempotence
    assert_eq!(
        report_1.recovered_stage_index,
        report_2.recovered_stage_index
    );
    assert_eq!(report_1.skipped_stages_count, report_2.skipped_stages_count);
    assert_eq!(
        report_1.recovered_tasks_count,
        report_2.recovered_tasks_count
    );
}
