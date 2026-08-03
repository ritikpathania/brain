use brain_services::planning::{
    EventLog, InMemoryEventLog, LogReplayEngine, NodeId, ReplicationEvent, ReplicationEventKind,
    ReplicationHealth, ReplicationHealthEvaluator, ReplicationProjection, SequenceNumber,
};
use uuid::Uuid;

#[test]
fn test_replication_health_evaluator_pure_policy_evaluation() {
    let node1 = NodeId(Uuid::new_v4());
    let mut metrics =
        brain_services::planning::FollowerReplicationMetrics::new(node1, 1000, SequenceNumber(10));

    // 1. Initial healthy state
    metrics.match_index = SequenceNumber(10);
    assert_eq!(
        ReplicationHealthEvaluator::evaluate_health(&metrics, SequenceNumber(10)),
        ReplicationHealth::Healthy
    );

    // 2. Lagging state (lag > 50)
    assert_eq!(
        ReplicationHealthEvaluator::evaluate_health(&metrics, SequenceNumber(70)),
        ReplicationHealth::Lagging
    );

    // 3. Backoff state (retry_count > 0)
    metrics.retry_count = 2;
    assert_eq!(
        ReplicationHealthEvaluator::evaluate_health(&metrics, SequenceNumber(10)),
        ReplicationHealth::Backoff
    );

    // 4. Snapshot required state
    metrics.snapshot_required = true;
    assert_eq!(
        ReplicationHealthEvaluator::evaluate_health(&metrics, SequenceNumber(10)),
        ReplicationHealth::SnapshotRequired
    );

    // 5. Offline state
    metrics.is_active = false;
    assert_eq!(
        ReplicationHealthEvaluator::evaluate_health(&metrics, SequenceNumber(10)),
        ReplicationHealth::Offline
    );
}

#[test]
fn test_replication_projection_event_replay_and_health_recovery() {
    let node1 = NodeId(Uuid::new_v4());
    let mut projection = ReplicationProjection::new();
    let log = InMemoryEventLog::<ReplicationEvent>::new();

    log.append(
        ReplicationEvent::new(
            1000,
            node1,
            ReplicationEventKind::WorkerRegistered {
                initial_next_index: SequenceNumber(1),
            },
        ),
        1000,
        1,
    )
    .unwrap();

    log.append(
        ReplicationEvent::new(
            1100,
            node1,
            ReplicationEventKind::BatchSent {
                start_sequence: SequenceNumber(1),
                entry_count: 5,
                bytes_count: 500,
            },
        ),
        1100,
        1,
    )
    .unwrap();

    log.append(
        ReplicationEvent::new(
            1200,
            node1,
            ReplicationEventKind::AckReceived {
                match_index: SequenceNumber(5),
                rtt_ms: 15,
            },
        ),
        1200,
        1,
    )
    .unwrap();

    log.append(
        ReplicationEvent::new(
            1300,
            node1,
            ReplicationEventKind::RetryScheduled {
                consecutive_failures: 1,
                backoff_ms: 50,
            },
        ),
        1300,
        1,
    )
    .unwrap();

    log.append(
        ReplicationEvent::new(1400, node1, ReplicationEventKind::ReplicationRecovered),
        1400,
        1,
    )
    .unwrap();

    LogReplayEngine::replay_from_offset(&log, &mut projection, SequenceNumber(1), 10).unwrap();

    assert_eq!(projection.last_applied_sequence(), SequenceNumber(5));
    let metrics = projection.get_metrics(&node1).unwrap();
    assert_eq!(metrics.match_index, SequenceNumber(5));
    assert_eq!(metrics.bytes_sent, 500);
    assert_eq!(metrics.ack_count, 1);
    assert_eq!(metrics.retry_count, 0); // Recovered back to 0!
    assert_eq!(
        projection.get_health(&node1, SequenceNumber(5)),
        ReplicationHealth::Healthy
    );

    // Verify time-windowed throughput calculation (500 bytes over 400ms = 1250 bytes/sec)
    assert_eq!(metrics.bytes_per_second(), 1250.0);
}

#[test]
fn test_replication_projection_replay_idempotency() {
    let node1 = NodeId(Uuid::new_v4());
    let mut proj1 = ReplicationProjection::new();
    let mut proj2 = ReplicationProjection::new();
    let log = InMemoryEventLog::<ReplicationEvent>::new();

    log.append(
        ReplicationEvent::new(
            1000,
            node1,
            ReplicationEventKind::WorkerRegistered {
                initial_next_index: SequenceNumber(1),
            },
        ),
        1000,
        1,
    )
    .unwrap();

    log.append(
        ReplicationEvent::new(
            1200,
            node1,
            ReplicationEventKind::AckReceived {
                match_index: SequenceNumber(10),
                rtt_ms: 10,
            },
        ),
        1200,
        1,
    )
    .unwrap();

    LogReplayEngine::replay_from_offset(&log, &mut proj1, SequenceNumber(1), 10).unwrap();

    // Replay twice on proj2
    LogReplayEngine::replay_from_offset(&log, &mut proj2, SequenceNumber(1), 10).unwrap();
    LogReplayEngine::replay_from_offset(&log, &mut proj2, SequenceNumber(1), 10).unwrap();

    assert_eq!(proj1, proj2);
}
