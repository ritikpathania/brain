//! Integration & Event-Sourced Projection Test Suite for `ReadEvent`, `ReadProjection`, and `ReadMetrics` (Phase 14 Milestone 14.2).

use brain_services::planning::{
    EventLog, InMemoryEventLog, LogReplayEngine, NodeId, ReadEvent, ReadEventKind,
    ReadEventPublisher, ReadPlanKind, ReadProjection, ReadValidationResult, ReplayTarget,
    SequenceNumber, TermId,
};
use uuid::Uuid;

#[test]
fn test_read_metrics_lazy_average_latency_calculation_zero_safe() {
    let mut proj = ReadProjection::new();

    // Zero successful reads -> zero safe (no divide-by-zero panic!)
    assert_eq!(proj.metrics.average_execution_latency_us(), 0.0);
    assert_eq!(proj.metrics.average_total_latency_us(), 0.0);

    let leader_id = NodeId(Uuid::new_v4());
    let read_id = Uuid::new_v4();
    let log = InMemoryEventLog::<ReadEvent>::new();

    log.append(
        ReadEvent::new(
            1000,
            read_id,
            leader_id,
            TermId(1),
            ReadEventKind::ReadRequested {
                target_read_index: SequenceNumber(10),
            },
        ),
        1000,
        1,
    )
    .unwrap();

    log.append(
        ReadEvent::new(
            1001,
            read_id,
            leader_id,
            TermId(1),
            ReadEventKind::ReadPlanCompiled {
                kind: ReadPlanKind::LeaseValidated,
                target_read_index: SequenceNumber(10),
            },
        ),
        1001,
        1,
    )
    .unwrap();

    log.append(
        ReadEvent::new(
            1002,
            read_id,
            leader_id,
            TermId(1),
            ReadEventKind::ReadServed {
                target_read_index: SequenceNumber(10),
                planning_latency_us: 100,
                validation_latency_us: 200,
                execution_latency_us: 300,
            },
        ),
        1002,
        1,
    )
    .unwrap();

    LogReplayEngine::replay_from_offset(&log, &mut proj, SequenceNumber(1), 10).unwrap();

    assert_eq!(proj.metrics.total_reads, 1);
    assert_eq!(proj.metrics.successful_reads, 1);
    assert_eq!(proj.metrics.rejected_reads, 0);
    assert_eq!(proj.metrics.average_execution_latency_us(), 300.0);
    assert_eq!(proj.metrics.average_total_latency_us(), 600.0); // 100 + 200 + 300 = 600
    assert_eq!(
        *proj
            .metrics
            .strategy_counts
            .get(&ReadPlanKind::LeaseValidated)
            .unwrap(),
        1
    );
}

#[test]
fn test_read_projection_rejection_tracking_and_strategy_map() {
    let mut proj = ReadProjection::new();
    let leader_id = NodeId(Uuid::new_v4());
    let read_id = Uuid::new_v4();
    let log = InMemoryEventLog::<ReadEvent>::new();

    log.append(
        ReadEvent::new(
            1000,
            read_id,
            leader_id,
            TermId(1),
            ReadEventKind::ReadRequested {
                target_read_index: SequenceNumber(5),
            },
        ),
        1000,
        1,
    )
    .unwrap();

    log.append(
        ReadEvent::new(
            1001,
            read_id,
            leader_id,
            TermId(1),
            ReadEventKind::ReadRejected {
                reason: ReadValidationResult::LeaseExpired,
            },
        ),
        1001,
        1,
    )
    .unwrap();

    LogReplayEngine::replay_from_offset(&log, &mut proj, SequenceNumber(1), 10).unwrap();

    assert_eq!(proj.metrics.total_reads, 1);
    assert_eq!(proj.metrics.successful_reads, 0);
    assert_eq!(proj.metrics.rejected_reads, 1);
    assert_eq!(
        *proj
            .metrics
            .rejections_by_reason
            .get(&ReadValidationResult::LeaseExpired)
            .unwrap(),
        1
    );
}

#[test]
fn test_read_projection_sequence_monotonicity_and_gap_detection() {
    let mut proj = ReadProjection::new();
    let leader_id = NodeId(Uuid::new_v4());
    let read_id = Uuid::new_v4();

    // 1. First event sequence 1
    let env1 = ReadEventPublisher::create_envelope(
        SequenceNumber(1),
        ReadEvent::new(
            1000,
            read_id,
            leader_id,
            TermId(1),
            ReadEventKind::ReadRequested {
                target_read_index: SequenceNumber(1),
            },
        ),
    );
    proj.apply_envelope(&env1);
    assert_eq!(proj.metrics.total_reads, 1);
    assert_eq!(proj.sequence_gaps, 0);

    // 2. Duplicate sequence 1 -> ignored silently for idempotency
    proj.apply_envelope(&env1);
    assert_eq!(proj.metrics.total_reads, 1);

    // 3. Sequence gap (jump from 1 to 5)
    let env5 = ReadEventPublisher::create_envelope(
        SequenceNumber(5),
        ReadEvent::new(
            1004,
            read_id,
            leader_id,
            TermId(1),
            ReadEventKind::ReadRequested {
                target_read_index: SequenceNumber(5),
            },
        ),
    );
    proj.apply_envelope(&env5);
    assert_eq!(proj.metrics.total_reads, 2);
    assert_eq!(proj.sequence_gaps, 1);

    // 4. Sequence regression (received sequence 3 when last was 5)
    let env3 = ReadEventPublisher::create_envelope(
        SequenceNumber(3),
        ReadEvent::new(
            1002,
            read_id,
            leader_id,
            TermId(1),
            ReadEventKind::ReadRequested {
                target_read_index: SequenceNumber(3),
            },
        ),
    );
    proj.apply_envelope(&env3);
    assert_eq!(proj.metrics.total_reads, 2); // Regress ignored!
    assert_eq!(proj.sequence_regressions, 1);
}
