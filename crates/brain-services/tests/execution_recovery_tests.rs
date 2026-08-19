use brain_services::runtime::*;
use brain_storage::Connection;

#[test]
fn test_recovery_engine_reconstructs_running_execution() {
    let conn = Connection::open_in_memory().unwrap();
    let repo = SqliteExecutionRepository::new(conn);
    repo.init_schema().unwrap();

    let exec_id = ExecutionId::new();
    let header = ExecutionHeader::new_root(exec_id);
    repo.create_execution(&header).unwrap();

    let event1 = JournalEvent {
        sequence_no: SequenceNo(1),
        execution_id: exec_id,
        version: ExecutionVersion(1),
        occurred_at: 100,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionEnqueued),
    };
    let event2 = JournalEvent {
        sequence_no: SequenceNo(2),
        execution_id: exec_id,
        version: ExecutionVersion(2),
        occurred_at: 105,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionBegan),
    };

    repo.append_journal_event(&event1).unwrap();
    repo.append_journal_event(&event2).unwrap();

    let engine = RecoveryEngine::new(repo);
    let projection = engine.recover_execution(exec_id).unwrap().unwrap();
    assert_eq!(projection.status, ExecutionFsmState::Running);
    assert_eq!(projection.version, ExecutionVersion(2));
}
