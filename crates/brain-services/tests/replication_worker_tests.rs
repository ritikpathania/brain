//! Integration & Flow Control Test Suite for `ReplicationWorker`, `ReplicationCoordinator`, and `ReplicationFlowController` (Phase 13 Milestone 13.1).

use brain_services::planning::{
    AppendEntriesResponse, EventEnvelope, FlowDecision, LeadershipEvent, LeadershipEventId,
    LeadershipEventKind, NodeId, ReplicationBatchKind, ReplicationCoordinator,
    ReplicationFlowController, ReplicationMeasurements, ReplicationTask, ReplicationWorker,
    ReplicationWorkerState, SequenceNumber, TermId, LEADERSHIP_EVENT_SCHEMA_VERSION,
};
use uuid::Uuid;

#[test]
fn test_replication_flow_controller_policy_evaluation() {
    let controller = ReplicationFlowController::new(1, 100);

    // 1. Low latency healthy network -> Max batch size
    let healthy_m = ReplicationMeasurements {
        rtt_ms: 10,
        ack_rate: 150.0,
        bytes_in_flight: 1024,
        consecutive_failures: 0,
    };
    let decision = controller.evaluate_flow(&healthy_m);
    assert_eq!(
        decision,
        FlowDecision {
            recommended_batch_size: 100,
            send_window: 200,
            pacing_delay_ms: 0,
            max_in_flight: 5,
        }
    );

    // 2. High latency network (RTT > 200ms) -> Min batch size + pacing delay
    let high_lat_m = ReplicationMeasurements {
        rtt_ms: 250,
        ack_rate: 10.0,
        bytes_in_flight: 4096,
        consecutive_failures: 0,
    };
    let lat_decision = controller.evaluate_flow(&high_lat_m);
    assert_eq!(lat_decision.recommended_batch_size, 1);
    assert_eq!(lat_decision.pacing_delay_ms, 20);

    // 3. Consecutive failures backoff -> Penalty delay
    let failure_m = ReplicationMeasurements {
        rtt_ms: 10,
        ack_rate: 0.0,
        bytes_in_flight: 0,
        consecutive_failures: 3,
    };
    let fail_decision = controller.evaluate_flow(&failure_m);
    assert_eq!(fail_decision.recommended_batch_size, 1);
    assert_eq!(fail_decision.pacing_delay_ms, 150); // 50 * 3 = 150ms
}

#[test]
fn test_replication_worker_lifecycle_and_response_processing() {
    let node1 = NodeId(Uuid::new_v4());
    let leader_id = NodeId(Uuid::new_v4());
    let mut worker = ReplicationWorker::new(node1, SequenceNumber(10));

    assert_eq!(worker.target_node(), node1);
    assert_eq!(worker.state(), ReplicationWorkerState::Idle);
    assert_eq!(worker.replication_state().next_index, SequenceNumber(11));
    assert_eq!(worker.replication_state().match_index, SequenceNumber(0));

    // 1. Create data batch
    let event = LeadershipEvent {
        schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
        event_id: LeadershipEventId(Uuid::new_v4()),
        kind: LeadershipEventKind::LeaderElectionStarted {
            candidates_count: 2,
        },
        timestamp_ms: 1000,
    };
    let envelope = EventEnvelope {
        sequence: SequenceNumber(11),
        timestamp_ms: 1000,
        schema_version: 1,
        payload: event,
    };

    let batch = worker.create_data_batch(SequenceNumber(11), vec![envelope]);
    assert_eq!(batch.kind, ReplicationBatchKind::Data);

    let task = worker.build_append_entries_task(
        &batch,
        TermId(1),
        leader_id,
        SequenceNumber(10),
        TermId(1),
        SequenceNumber(10),
    );

    match task {
        ReplicationTask::AppendEntries(req) => {
            assert_eq!(req.term, TermId(1));
            assert_eq!(req.entries.len(), 1);
        }
        _ => panic!("Expected AppendEntries task"),
    }

    // 2. Process successful ACK
    worker.set_state(ReplicationWorkerState::WaitingAck);
    let success_resp = AppendEntriesResponse {
        term: TermId(1),
        success: true,
        match_index: SequenceNumber(11),
        reject_reason: None,
    };
    worker.process_response(&success_resp);

    assert_eq!(worker.state(), ReplicationWorkerState::Idle);
    assert_eq!(worker.replication_state().match_index, SequenceNumber(11));
    assert_eq!(worker.replication_state().next_index, SequenceNumber(12));

    // 3. Process failed ACK (log mismatch)
    let fail_resp = AppendEntriesResponse {
        term: TermId(1),
        success: false,
        match_index: SequenceNumber(0),
        reject_reason: Some(brain_services::planning::AppendEntriesRejectReason::LogMismatch),
    };
    worker.process_response(&fail_resp);

    assert_eq!(worker.state(), ReplicationWorkerState::BackingOff);
    assert_eq!(worker.replication_state().next_index, SequenceNumber(11));
}

#[test]
fn test_replication_coordinator_multi_follower_management() {
    let coordinator = ReplicationCoordinator::new();
    let node1 = NodeId(Uuid::new_v4());
    let node2 = NodeId(Uuid::new_v4());

    coordinator.register_follower(node1, SequenceNumber(5));
    coordinator.register_follower(node2, SequenceNumber(5));

    let registered = coordinator.registered_followers();
    assert_eq!(registered.len(), 2);
    assert!(registered.contains(&node1));
    assert!(registered.contains(&node2));

    coordinator.with_worker(&node1, |worker| {
        assert_eq!(worker.replication_state().next_index, SequenceNumber(6));
        worker.set_state(ReplicationWorkerState::Sending);
    });

    coordinator.with_worker(&node1, |worker| {
        assert_eq!(worker.state(), ReplicationWorkerState::Sending);
    });

    coordinator.deregister_follower(&node1);
    assert_eq!(coordinator.registered_followers().len(), 1);
}
