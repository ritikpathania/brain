//! Integration & Storage Durability Test Suite for `SqliteEventLog` & `ConsensusStorage` (Phase 12 Milestone 12.1).

use brain_services::planning::{
    ConsensusPersistentState, ConsensusStorage, EventCodec, EventLog, InMemoryConsensusStorage,
    JsonEventCodec, LeadershipEvent, LeadershipEventId, LeadershipEventKind, LeadershipProjection,
    LogReplayEngine, NodeId, SequenceNumber, SqliteEventLog, TermId,
    LEADERSHIP_EVENT_SCHEMA_VERSION,
};
use std::sync::Arc;
use std::thread;
use tempfile::NamedTempFile;
use uuid::Uuid;

#[test]
fn test_json_event_codec_encode_decode() {
    let codec = JsonEventCodec::<LeadershipEvent>::new();
    let event = LeadershipEvent {
        schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
        event_id: LeadershipEventId(Uuid::new_v4()),
        kind: LeadershipEventKind::LeaderElectionStarted {
            candidates_count: 3,
        },
        timestamp_ms: 12345678,
    };

    let encoded = codec.encode(&event).unwrap();
    assert!(!encoded.is_empty());

    let decoded = codec.decode(&encoded).unwrap();
    assert_eq!(decoded, event);
}

#[test]
fn test_sqlite_event_log_atomic_appends_and_sequence_monotonicity() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();

    let codec = JsonEventCodec::<LeadershipEvent>::new();
    let log = SqliteEventLog::new(db_path, codec).unwrap();

    let event1 = LeadershipEvent {
        schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
        event_id: LeadershipEventId(Uuid::new_v4()),
        kind: LeadershipEventKind::LeaderElectionStarted {
            candidates_count: 2,
        },
        timestamp_ms: 1000,
    };
    let event2 = LeadershipEvent {
        schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
        event_id: LeadershipEventId(Uuid::new_v4()),
        kind: LeadershipEventKind::LeaderElected {
            leader_id: NodeId(Uuid::new_v4()),
            epoch: brain_services::planning::EpochId(1),
        },
        timestamp_ms: 1001,
    };

    let seq1 = log.append(event1.clone(), 1000, 1).unwrap();
    let seq2 = log.append(event2.clone(), 1001, 1).unwrap();

    assert_eq!(seq1, SequenceNumber(1));
    assert_eq!(seq2, SequenceNumber(2));
    assert_eq!(log.last_sequence_number(), SequenceNumber(2));

    let envelopes = log.read_range(SequenceNumber(1), 10).unwrap();
    assert_eq!(envelopes.len(), 2);
    assert_eq!(envelopes[0].sequence, SequenceNumber(1));
    assert_eq!(envelopes[0].payload, event1);
    assert_eq!(envelopes[1].sequence, SequenceNumber(2));
    assert_eq!(envelopes[1].payload, event2);
}

#[test]
fn test_sqlite_event_log_concurrent_appends() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();

    let codec = JsonEventCodec::<LeadershipEvent>::new();
    let log = Arc::new(SqliteEventLog::new(db_path, codec).unwrap());
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
                timestamp_ms: 2000 + i as u64,
            };
            log_clone.append(event, 2000 + i as u64, 1).unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(log.last_sequence_number(), SequenceNumber(10));
    let envelopes = log.read_range(SequenceNumber(1), 10).unwrap();
    assert_eq!(envelopes.len(), 10);
}

#[test]
fn test_sqlite_event_log_replay_into_projection() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();

    let codec = JsonEventCodec::<LeadershipEvent>::new();
    let log = SqliteEventLog::new(db_path, codec).unwrap();

    let event = LeadershipEvent {
        schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
        event_id: LeadershipEventId(Uuid::new_v4()),
        kind: LeadershipEventKind::LeaderElectionStarted {
            candidates_count: 5,
        },
        timestamp_ms: 3000,
    };
    log.append(event, 3000, 1).unwrap();

    let mut projection = LeadershipProjection::new();
    let count =
        LogReplayEngine::replay_from_offset(&log, &mut projection, SequenceNumber(1), 100).unwrap();

    assert_eq!(count, 1);
    assert_eq!(projection.total_events, 1);
    assert_eq!(projection.election_started_count, 1);
}

#[test]
fn test_consensus_storage_state_persistence() {
    let storage = InMemoryConsensusStorage::new();
    let initial_state = storage.load_state().unwrap();
    assert_eq!(initial_state, ConsensusPersistentState::default());

    let node_id = NodeId(Uuid::new_v4());
    let state = ConsensusPersistentState {
        current_term: TermId(5),
        voted_for: Some(node_id),
        schema_version: 1,
    };

    storage.save_state(&state).unwrap();
    let loaded = storage.load_state().unwrap();
    assert_eq!(loaded, state);
}
