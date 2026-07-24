use brain_services::runtime::*;
use rusqlite::Connection;

#[test]
fn test_sqlite_execution_repository_contract() {
    let conn = Connection::open_in_memory().unwrap();
    let repo = SqliteExecutionRepository::new(conn);
    repo.init_schema().unwrap();

    let exec_id = ExecutionId::new();
    let header = ExecutionHeader::new_root(exec_id);

    repo.create_execution(&header).unwrap();
    let loaded = repo.get_execution_header(exec_id).unwrap().unwrap();
    assert_eq!(loaded.execution_id, exec_id);

    let event = JournalEvent {
        sequence_no: SequenceNo(1),
        execution_id: exec_id,
        version: ExecutionVersion(1),
        occurred_at: 1000,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionEnqueued),
    };
    repo.append_journal_event(&event).unwrap();

    let events = repo.get_journal_events(exec_id, SequenceNo(0)).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], event);
}
