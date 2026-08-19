use brain_services::runtime::*;
use brain_storage::Connection;

#[test]
fn test_failure_injection_crash_and_recovery_replay() {
    let conn = Connection::open_in_memory().unwrap();
    let repo = SqliteExecutionRepository::new(conn);
    repo.init_schema().unwrap();

    let exec_id = ExecutionId::new();
    let task_id = TaskId::new();
    let header = ExecutionHeader::new_root(exec_id);

    repo.create_execution(&header).unwrap();

    // 1. Simulate workflow start
    let events = vec![
        JournalEvent {
            sequence_no: SequenceNo(1),
            execution_id: exec_id,
            version: ExecutionVersion(1),
            occurred_at: 1000,
            payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionEnqueued),
        },
        JournalEvent {
            sequence_no: SequenceNo(2),
            execution_id: exec_id,
            version: ExecutionVersion(2),
            occurred_at: 1001,
            payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionBegan),
        },
        JournalEvent {
            sequence_no: SequenceNo(3),
            execution_id: exec_id,
            version: ExecutionVersion(3),
            occurred_at: 1002,
            payload: JournalPayload::Task(TaskEventPayload::TaskCreated {
                task_id,
                job_id: brain_domain::jobs::JobId(uuid::Uuid::new_v4()),
                priority: 1,
            }),
        },
        JournalEvent {
            sequence_no: SequenceNo(4),
            execution_id: exec_id,
            version: ExecutionVersion(4),
            occurred_at: 1003,
            payload: JournalPayload::Task(TaskEventPayload::TaskLeased {
                task_id,
                worker_id: "worker-1".to_string(),
                lease_until: 1050,
            }),
        },
    ];

    for ev in &events {
        repo.append_journal_event(ev).unwrap();
    }

    // 2. Simulate process crash mid-task & recover
    let engine = RecoveryEngine::new(repo);
    let recovered = engine.recover_execution(exec_id).unwrap().unwrap();

    assert_eq!(recovered.status, ExecutionFsmState::Running);
    assert_eq!(recovered.version, ExecutionVersion(4));
    let task_snap = recovered.tasks.get(&task_id).unwrap();
    assert_eq!(task_snap.status, TaskFsmState::Leased);
    assert_eq!(task_snap.lease_owner.as_deref(), Some("worker-1"));
}

#[test]
fn test_failure_injection_duplicate_replay_safety() {
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
        occurred_at: 1000,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionEnqueued),
    };
    repo.append_journal_event(&event1).unwrap();

    let engine = RecoveryEngine::new(repo);
    let recovered_first = engine.recover_execution(exec_id).unwrap().unwrap();
    let recovered_second = engine.recover_execution(exec_id).unwrap().unwrap();

    assert_eq!(recovered_first, recovered_second);
}
