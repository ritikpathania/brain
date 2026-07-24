use brain_services::runtime::*;

#[test]
fn test_execution_aggregator_verifies_event_version() {
    let exec_id = ExecutionId::new();
    let header = ExecutionHeader::new_root(exec_id);
    let mut aggregator = ExecutionAggregator::new(header);

    let event1 = JournalEvent {
        sequence_no: SequenceNo(1),
        execution_id: exec_id,
        version: ExecutionVersion(1),
        occurred_at: 100,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionEnqueued),
    };
    aggregator.apply(&event1).unwrap();

    let invalid_version_event = JournalEvent {
        sequence_no: SequenceNo(2),
        execution_id: exec_id,
        version: ExecutionVersion(5), // Unexpected version jump
        occurred_at: 105,
        payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionBegan),
    };
    assert!(aggregator.apply(&invalid_version_event).is_err());
}

#[test]
fn test_execution_aggregator_deterministic_replay() {
    let exec_id = ExecutionId::new();
    let header = ExecutionHeader::new_root(exec_id);
    let mut aggregator = ExecutionAggregator::new(header);

    let events = vec![
        JournalEvent {
            sequence_no: SequenceNo(1),
            execution_id: exec_id,
            version: ExecutionVersion(1),
            occurred_at: 100,
            payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionEnqueued),
        },
        JournalEvent {
            sequence_no: SequenceNo(2),
            execution_id: exec_id,
            version: ExecutionVersion(2),
            occurred_at: 105,
            payload: JournalPayload::Execution(ExecutionEventPayload::ExecutionBegan),
        },
    ];

    for event in &events {
        aggregator.apply(event).unwrap();
    }

    let projection = aggregator.projection();
    assert_eq!(projection.status, ExecutionFsmState::Running);
    assert_eq!(projection.version, ExecutionVersion(2));
}
