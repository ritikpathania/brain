//! Integration & Recovery Test Suite for `EventLog<E>` & `LogReplayEngine` (Phase 11 Milestone 11.1).

use brain_services::planning::{
    ClusterEvent, ClusterEventKind, ClusterTopologyProjection, EventLog, InMemoryEventLog,
    LeadershipEvent, LeadershipEventId, LeadershipEventKind, LeadershipProjection, LogReplayEngine,
    NodeId, SequenceNumber, LEADERSHIP_EVENT_SCHEMA_VERSION,
};
use std::sync::Arc;
use std::thread;
use uuid::Uuid;

#[test]
fn test_event_log_concurrent_append_and_sequence_monotonicity() {
    let log = Arc::new(InMemoryEventLog::<LeadershipEvent>::new());
    let mut handles = Vec::new();

    for i in 0..10 {
        let log_clone = log.clone();
        let handle = thread::spawn(move || {
            let event = LeadershipEvent {
                schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
                event_id: LeadershipEventId(Uuid::new_v4()),
                kind: LeadershipEventKind::LeaderElectionStarted {
                    candidates_count: i,
                },
                timestamp_ms: 1000 + i as u64,
            };
            log_clone
                .append(event, 1000 + i as u64, LEADERSHIP_EVENT_SCHEMA_VERSION)
                .unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(log.last_sequence_number(), SequenceNumber(10));
    let envelopes = log.read_range(SequenceNumber(1), 10).unwrap();
    assert_eq!(envelopes.len(), 10);

    // Verify sequence numbers are monotonic and gap-free (1..=10)
    for (idx, env) in envelopes.iter().enumerate() {
        assert_eq!(env.sequence, SequenceNumber((idx + 1) as u64));
    }
}

#[test]
fn test_log_replay_engine_empty_log_recovery() {
    let log = InMemoryEventLog::<LeadershipEvent>::new();
    let mut projection = LeadershipProjection::new();

    let replayed =
        LogReplayEngine::replay_from_offset(&log, &mut projection, SequenceNumber(1), 100).unwrap();
    assert_eq!(replayed, 0);
    assert_eq!(projection.total_events, 0);
}

#[test]
fn test_log_replay_engine_arbitrary_offset_and_chunking_determinism() {
    let log = InMemoryEventLog::<LeadershipEvent>::new();
    let leader_id = NodeId(Uuid::new_v4());

    let ev1 = LeadershipEvent {
        schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
        event_id: LeadershipEventId(Uuid::new_v4()),
        kind: LeadershipEventKind::LeaderElectionStarted {
            candidates_count: 3,
        },
        timestamp_ms: 1000,
    };
    let ev2 = LeadershipEvent {
        schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
        event_id: LeadershipEventId(Uuid::new_v4()),
        kind: LeadershipEventKind::LeaderElected {
            leader_id,
            epoch: brain_services::planning::EpochId(1),
        },
        timestamp_ms: 1001,
    };
    let ev3 = LeadershipEvent {
        schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
        event_id: LeadershipEventId(Uuid::new_v4()),
        kind: LeadershipEventKind::LeadershipLost {
            former_leader_id: leader_id,
            epoch: brain_services::planning::EpochId(1),
        },
        timestamp_ms: 1002,
    };

    log.append(ev1, 1000, 1).unwrap();
    log.append(ev2, 1001, 1).unwrap();
    log.append(ev3, 1002, 1).unwrap();

    // 1. Single-pass full replay
    let mut proj1 = LeadershipProjection::new();
    let count1 =
        LogReplayEngine::replay_from_offset(&log, &mut proj1, SequenceNumber(1), 100).unwrap();
    assert_eq!(count1, 3);
    assert_eq!(proj1.total_events, 3);
    assert_eq!(proj1.election_started_count, 1);
    assert_eq!(proj1.leaders_elected_count, 1);
    assert_eq!(proj1.losses_count, 1);

    // 2. Chunked replay (batch size = 1) yields IDENTICAL projection state
    let mut proj2 = LeadershipProjection::new();
    let count2 =
        LogReplayEngine::replay_from_offset(&log, &mut proj2, SequenceNumber(1), 1).unwrap();
    assert_eq!(count2, 3);
    assert_eq!(proj1, proj2);

    // 3. Incremental replay from arbitrary offset SequenceNumber(2)
    let mut proj3 = LeadershipProjection::new();
    let count3 =
        LogReplayEngine::replay_from_offset(&log, &mut proj3, SequenceNumber(2), 100).unwrap();
    assert_eq!(count3, 2);
    assert_eq!(proj3.total_events, 2);
    assert_eq!(proj3.leaders_elected_count, 1);
    assert_eq!(proj3.losses_count, 1);
}

#[test]
fn test_cluster_topology_projection_log_replay() {
    let log = InMemoryEventLog::<ClusterEvent>::new();
    let node_id = NodeId(Uuid::new_v4());

    let ev = ClusterEvent {
        event_id: brain_services::planning::ClusterEventId(Uuid::new_v4()),
        kind: ClusterEventKind::NodeJoined,
        message: format!("Node {} joined cluster", node_id),
        timestamp_ms: 5000,
    };

    log.append(ev, 5000, 1).unwrap();

    let mut proj = ClusterTopologyProjection::new();
    LogReplayEngine::replay_from_offset(&log, &mut proj, SequenceNumber(1), 10).unwrap();

    assert_eq!(proj.nodes_joined_count, 1);
    assert_eq!(proj.total_events, 1);
}
