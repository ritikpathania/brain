//! Integration & Telemetry Test Suite for `ClusterTelemetryProjection`, `SlaSloMonitor`, and `ClusterHealthDashboard` (Phase 15 Milestone 15.3).

use brain_services::planning::{
    ClusterHealthDashboard, ClusterTelemetryProjection, EventLog, InMemoryEventLog,
    LogReplayEngine, NodeId, ReadEvent, ReadEventKind, SequenceNumber, SlaSloMonitor, SloPolicy,
    TermId,
};
use uuid::Uuid;

#[test]
fn test_cluster_telemetry_projection_event_replay_and_lazy_metrics() {
    let mut proj = ClusterTelemetryProjection::new();
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
                target_read_index: SequenceNumber(1),
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
            ReadEventKind::ReadServed {
                target_read_index: SequenceNumber(1),
                planning_latency_us: 500,
                validation_latency_us: 1000,
                execution_latency_us: 1500,
            },
        ),
        1001,
        1,
    )
    .unwrap();

    LogReplayEngine::replay_from_offset(&log, &mut proj, SequenceNumber(1), 10).unwrap();

    assert_eq!(proj.metrics.total_requests, 1);
    assert_eq!(proj.metrics.successful_requests, 1);
    assert_eq!(proj.metrics.failed_requests, 0);
    assert_eq!(proj.metrics.availability_pct(), 100.0);
    assert_eq!(proj.metrics.average_latency_us(), 3000.0); // 500 + 1000 + 1500 = 3000
}

#[test]
fn test_sla_slo_monitor_evaluation_and_error_budget_depletion() {
    let mut proj = ClusterTelemetryProjection::new();
    let leader_id = NodeId(Uuid::new_v4());
    let read_id = Uuid::new_v4();
    let log = InMemoryEventLog::<ReadEvent>::new();

    // Append 99 successful requests and 1 failed request -> 99.0% availability
    for i in 1..=99 {
        log.append(
            ReadEvent::new(
                1000 + i,
                read_id,
                leader_id,
                TermId(1),
                ReadEventKind::ReadRequested {
                    target_read_index: SequenceNumber(i),
                },
            ),
            1000 + i,
            1,
        )
        .unwrap();

        log.append(
            ReadEvent::new(
                1000 + i,
                read_id,
                leader_id,
                TermId(1),
                ReadEventKind::ReadServed {
                    target_read_index: SequenceNumber(i),
                    planning_latency_us: 100,
                    validation_latency_us: 100,
                    execution_latency_us: 100,
                },
            ),
            1000 + i,
            1,
        )
        .unwrap();
    }

    // 1 failed request
    log.append(
        ReadEvent::new(
            2000,
            read_id,
            leader_id,
            TermId(1),
            ReadEventKind::ReadRequested {
                target_read_index: SequenceNumber(100),
            },
        ),
        2000,
        1,
    )
    .unwrap();

    log.append(
        ReadEvent::new(
            2001,
            read_id,
            leader_id,
            TermId(1),
            ReadEventKind::ReadRejected {
                reason: brain_services::planning::ReadValidationResult::LeaseExpired,
            },
        ),
        2001,
        1,
    )
    .unwrap();

    LogReplayEngine::replay_from_offset(&log, &mut proj, SequenceNumber(1), 500).unwrap();

    let policy = SloPolicy {
        target_availability_pct: 99.0, // Target = 99.0%
        max_average_latency_us: 50000.0,
    };

    let report = SlaSloMonitor::evaluate_slo(&proj.metrics, &policy);

    assert_eq!(report.actual_availability_pct, 99.0);
    assert!(report.slo_met);
    assert_eq!(report.error_budget_remaining_pct, 0.0); // 1.0% failure allowed, 1.0% actual -> 0% remaining budget!

    let dashboard = ClusterHealthDashboard::compile(proj.metrics, &policy);
    assert_eq!(dashboard.slo_report, report);
}
