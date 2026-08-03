//! Integration & Log Compaction Test Suite for `LogSnapshot`, `SnapshotStore`, `CompactionPlanner`, and `RecoveryEngine` (Phase 12 Milestone 12.2).

use brain_services::planning::{
    CompactionExecutor, CompactionPlan, CompactionPlanner, EventLog, InMemoryEventLog,
    InMemorySnapshotStore, JsonSnapshotCodec, LeadershipEvent, LeadershipEventId,
    LeadershipEventKind, LeadershipProjection, LogReplayEngine, RecoveryEngine, SequenceNumber,
    SnapshotBuilder, SnapshotStore, TermId, LEADERSHIP_EVENT_SCHEMA_VERSION,
};
use uuid::Uuid;

#[test]
fn test_snapshot_builder_self_validating_checksum() {
    let codec = JsonSnapshotCodec::<LeadershipProjection>::new();
    let mut proj = LeadershipProjection::new();
    proj.total_events = 10;
    proj.election_started_count = 5;

    let snapshot =
        SnapshotBuilder::build_snapshot(&proj, SequenceNumber(10), TermId(2), &codec, 1, 10000)
            .unwrap();

    assert_eq!(snapshot.snapshot_sequence, SequenceNumber(10));
    assert_eq!(snapshot.snapshot_term, TermId(2));
    assert!(snapshot.verify_checksum());

    // Corrupt snapshot state payload
    let mut corrupted = snapshot.clone();
    corrupted.state_payload[0] ^= 0xFF;
    assert!(!corrupted.verify_checksum());
}

#[test]
fn test_compaction_planner_and_executor_stable_sequence_numbers() {
    let log = InMemoryEventLog::<LeadershipEvent>::new();
    for i in 1..=10 {
        let event = LeadershipEvent {
            schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
            event_id: LeadershipEventId(Uuid::new_v4()),
            kind: LeadershipEventKind::LeaderElectionStarted {
                candidates_count: i,
            },
            timestamp_ms: 1000 + i as u64,
        };
        log.append(event, 1000 + i as u64, 1).unwrap();
    }

    assert_eq!(log.last_sequence_number(), SequenceNumber(10));

    let plan = CompactionPlanner::plan_compaction(&log, SequenceNumber(5)).unwrap();
    assert_eq!(
        plan,
        CompactionPlan {
            cutoff_sequence: SequenceNumber(5),
            retained_range_start: SequenceNumber(6),
        }
    );

    let truncated_count = CompactionExecutor::execute_compaction(&log, &plan).unwrap();
    assert_eq!(truncated_count, 5);

    // Verify stable sequence numbers post-compaction: range starting at SequenceNumber(6)
    let envelopes = log.read_range(plan.retained_range_start, 10).unwrap();
    assert_eq!(envelopes.len(), 5);
    assert_eq!(envelopes[0].sequence, SequenceNumber(6));
    assert_eq!(envelopes[4].sequence, SequenceNumber(10));
}

#[test]
fn test_recovery_engine_incremental_snapshot_then_replay_equivalence() {
    let log = InMemoryEventLog::<LeadershipEvent>::new();
    let snapshot_store = InMemorySnapshotStore::new();
    let snapshot_codec = JsonSnapshotCodec::<LeadershipProjection>::new();

    // 1. Append 10 events into log
    for i in 1..=10 {
        let event = LeadershipEvent {
            schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
            event_id: LeadershipEventId(Uuid::new_v4()),
            kind: LeadershipEventKind::LeaderElectionStarted {
                candidates_count: i,
            },
            timestamp_ms: 1000 + i as u64,
        };
        log.append(event, 1000 + i as u64, 1).unwrap();
    }

    // 2. Full uncompacted log replay state
    let mut full_proj = LeadershipProjection::new();
    LogReplayEngine::replay_from_offset(&log, &mut full_proj, SequenceNumber(1), 100).unwrap();
    assert_eq!(full_proj.total_events, 10);
    assert_eq!(full_proj.election_started_count, 10);

    // 3. Build and save snapshot at SequenceNumber(5)
    let mut snapshot_proj = LeadershipProjection::new();
    LogReplayEngine::replay_range(&log, &mut snapshot_proj, SequenceNumber(1), 5).unwrap();
    assert_eq!(snapshot_proj.total_events, 5);

    let snapshot = SnapshotBuilder::build_snapshot(
        &snapshot_proj,
        SequenceNumber(5),
        TermId(1),
        &snapshot_codec,
        1,
        5000,
    )
    .unwrap();
    snapshot_store.save_snapshot(&snapshot).unwrap();

    // 4. Incremental recovery via RecoveryEngine
    let mut recovered_proj = LeadershipProjection::new();
    let (replayed_count, restored) = RecoveryEngine::recover::<
        LeadershipProjection,
        LeadershipEvent,
        LeadershipProjection,
        JsonSnapshotCodec<LeadershipProjection>,
        InMemoryEventLog<LeadershipEvent>,
        InMemorySnapshotStore,
    >(
        &snapshot_store,
        &snapshot_codec,
        &log,
        &mut recovered_proj,
        100,
    )
    .unwrap();

    assert!(restored);
    assert_eq!(replayed_count, 5); // Replayed events 6..=10
    assert_eq!(recovered_proj, full_proj); // EQUIVALENCE INVARIANT: RestoreSnapshot(5) + Replay(6..10) == Replay(1..10)
}
