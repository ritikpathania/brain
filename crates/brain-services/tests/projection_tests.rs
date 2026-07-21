use brain_core::errors::BrainError;
use brain_events::{DomainEvent, EventEnvelope, EventLog, SystemEvent};
use brain_services::{
    JobProjectionReducer, ProjectionId, ProjectionNotificationBus, ProjectionRunner,
    SessionProjectionReducer, StateReducer, SystemEventLog,
};
use brain_storage::{
    ReadModelRepository, SqliteEventLog, SqliteJobReadModelRepository,
    SqliteProjectionCheckpointRepository, SqliteSessionReadModelRepository, TestStorage,
};
use parking_lot::Mutex;
use std::sync::Arc;
use uuid::Uuid;

struct TestReducer {
    id: ProjectionId,
    processed: Arc<Mutex<Vec<u64>>>,
    fail_on_seq: Arc<Mutex<Option<u64>>>,
    reset_called: Arc<Mutex<bool>>,
}

impl TestReducer {
    fn new(id: ProjectionId) -> Self {
        Self {
            id,
            processed: Arc::new(Mutex::new(Vec::new())),
            fail_on_seq: Arc::new(Mutex::new(None)),
            reset_called: Arc::new(Mutex::new(false)),
        }
    }
}

impl StateReducer for TestReducer {
    fn id(&self) -> ProjectionId {
        self.id
    }

    fn version(&self) -> u32 {
        1
    }

    fn reduce(&self, _conn: &rusqlite::Connection, envelope: &EventEnvelope) -> Result<(), BrainError> {
        let seq = envelope.sequence.unwrap();
        if let Some(fail_seq) = *self.fail_on_seq.lock() {
            if seq == fail_seq {
                return Err(BrainError::Storage {
                    message: "Simulated reducer failure".to_string(),
                    source: None,
                });
            }
        }
        self.processed.lock().push(seq);
        Ok(())
    }

    fn reset(&self, _conn: &rusqlite::Connection) -> Result<(), BrainError> {
        *self.reset_called.lock() = true;
        self.processed.lock().clear();
        Ok(())
    }
}

fn create_envelope(source: &str, val: u64) -> EventEnvelope {
    let payload = DomainEvent::System(SystemEvent::ConfigReloaded {
        keys_changed: vec![format!("key_{}", val)],
    });
    EventEnvelope::new(source.to_string(), payload)
}

#[test]
fn test_duplicate_registration_rejection() {
    let test_storage = TestStorage::new();
    let raw_log = Arc::new(SqliteEventLog::new(test_storage.store().pool().clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(
        test_storage.store().pool().clone(),
    ));
    let runner = ProjectionRunner::new(
        event_log,
        checkpoint_repo,
        Arc::new(ProjectionNotificationBus::new()),
    );

    let reducer1 = Arc::new(TestReducer::new(ProjectionId::TestA));
    let reducer2 = Arc::new(TestReducer::new(ProjectionId::TestA));

    assert!(runner.register(reducer1.clone()).is_ok());
    assert!(runner.register(reducer2.clone()).is_err());
}

#[test]
fn test_idempotency_and_catch_up() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool));
    let runner = ProjectionRunner::new(
        event_log.clone(),
        checkpoint_repo.clone(),
        Arc::new(ProjectionNotificationBus::new()),
    );

    let reducer = TestReducer::new(ProjectionId::TestA);
    let processed = reducer.processed.clone();
    runner.register(Arc::new(reducer)).unwrap();

    // 1. Append 3 events
    event_log.append(&create_envelope("test", 1)).unwrap();
    event_log.append(&create_envelope("test", 2)).unwrap();
    event_log.append(&create_envelope("test", 3)).unwrap();

    // 2. Catch up first time
    runner.catch_up().unwrap();
    assert_eq!(*processed.lock(), vec![1, 2, 3]);
    assert_eq!(checkpoint_repo.get_checkpoint("test_a").unwrap(), 3);

    // 3. Catch up again - should do nothing
    runner.catch_up().unwrap();
    assert_eq!(*processed.lock(), vec![1, 2, 3]);
}

#[test]
fn test_reducer_failure_resumption() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool));
    let runner = ProjectionRunner::new(
        event_log.clone(),
        checkpoint_repo.clone(),
        Arc::new(ProjectionNotificationBus::new()),
    );

    let reducer = TestReducer::new(ProjectionId::TestA);
    let processed = reducer.processed.clone();
    let fail_on_seq = reducer.fail_on_seq.clone();
    runner.register(Arc::new(reducer)).unwrap();

    // Append 3 events
    event_log.append(&create_envelope("test", 1)).unwrap();
    event_log.append(&create_envelope("test", 2)).unwrap();
    event_log.append(&create_envelope("test", 3)).unwrap();

    // Fail on sequence 2
    *fail_on_seq.lock() = Some(2);

    let res = runner.catch_up();
    assert!(res.is_err());

    // Checkpoint should be at 1 (since 1 succeeded but 2 failed)
    assert_eq!(checkpoint_repo.get_checkpoint("test_a").unwrap(), 1);
    assert_eq!(*processed.lock(), vec![1]);

    // Resolve failure and rerun
    *fail_on_seq.lock() = None;
    runner.catch_up().unwrap();

    // Verify it resumed from 2 and finished all events
    assert_eq!(checkpoint_repo.get_checkpoint("test_a").unwrap(), 3);
    assert_eq!(*processed.lock(), vec![1, 2, 3]);
}

#[test]
fn test_independent_checkpoints() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool));
    let runner = ProjectionRunner::new(
        event_log.clone(),
        checkpoint_repo.clone(),
        Arc::new(ProjectionNotificationBus::new()),
    );

    let reducer_a = TestReducer::new(ProjectionId::TestA);
    let processed_a = reducer_a.processed.clone();

    let reducer_b = TestReducer::new(ProjectionId::TestB);
    let fail_on_seq_b = reducer_b.fail_on_seq.clone();

    runner.register(Arc::new(reducer_a)).unwrap();
    runner.register(Arc::new(reducer_b)).unwrap();

    // Append 3 events
    event_log.append(&create_envelope("test", 1)).unwrap();
    event_log.append(&create_envelope("test", 2)).unwrap();
    event_log.append(&create_envelope("test", 3)).unwrap();

    // Fail B on sequence 2
    *fail_on_seq_b.lock() = Some(2);

    let res = runner.catch_up();
    assert!(res.is_err());

    // Checkpoints: A should be 3, B should be 1
    assert_eq!(checkpoint_repo.get_checkpoint("test_a").unwrap(), 3);
    assert_eq!(checkpoint_repo.get_checkpoint("test_b").unwrap(), 1);
    assert_eq!(*processed_a.lock(), vec![1, 2, 3]);
}

#[test]
fn test_deterministic_rebuild() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool));
    let runner = ProjectionRunner::new(
        event_log.clone(),
        checkpoint_repo.clone(),
        Arc::new(ProjectionNotificationBus::new()),
    );

    let reducer = TestReducer::new(ProjectionId::TestA);
    let processed = reducer.processed.clone();
    let reset_called = reducer.reset_called.clone();
    runner.register(Arc::new(reducer)).unwrap();

    // Append 10 events
    for i in 1..=10 {
        event_log.append(&create_envelope("test", i)).unwrap();
    }

    runner.catch_up().unwrap();
    assert_eq!(processed.lock().len(), 10);
    assert_eq!(checkpoint_repo.get_checkpoint("test_a").unwrap(), 10);

    // Call rebuild
    runner.rebuild_projection(ProjectionId::TestA).unwrap();

    // Verify reset was called, and processed holds exact 1..=10 sequence in order
    assert!(*reset_called.lock());
    assert_eq!(*processed.lock(), (1..=10).collect::<Vec<u64>>());
}

#[test]
fn test_empty_log_rebuild() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool));
    let runner = ProjectionRunner::new(
        event_log,
        checkpoint_repo.clone(),
        Arc::new(ProjectionNotificationBus::new()),
    );

    let reducer = TestReducer::new(ProjectionId::TestA);
    runner.register(Arc::new(reducer)).unwrap();

    assert!(runner.rebuild_projection(ProjectionId::TestA).is_ok());
    assert_eq!(checkpoint_repo.get_checkpoint("test_a").unwrap(), 0);

    assert!(runner.rebuild_all().is_ok());
}

#[test]
fn test_job_projection_parity_rebuild_and_interruption() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool.clone()));
    let runner = ProjectionRunner::new(
        event_log.clone(),
        checkpoint_repo.clone(),
        Arc::new(ProjectionNotificationBus::new()),
    );

    let job_repo = Arc::new(SqliteJobReadModelRepository::new(pool.clone()));
    let reducer = JobProjectionReducer::new(job_repo.clone());
    runner.register(Arc::new(reducer)).unwrap();

    let job_id = brain_domain::jobs::JobId(Uuid::new_v4());

    // Simulate domain events emitted sequentially for a single Job:
    // 1. JobCreated
    let ev1 = DomainEvent::Core(brain_domain::DomainEvent::JobCreated {
        job_id,
        kind: brain_domain::jobs::JobKind::Indexing,
        priority: brain_domain::jobs::JobPriority::High,
        owner: brain_domain::jobs::JobOwner::System,
    });
    event_log
        .append(&EventEnvelope::new("scheduler".to_string(), ev1))
        .unwrap(); // Seq 1

    // 2. JobStarted
    let ev2 = DomainEvent::Core(brain_domain::DomainEvent::JobStarted {
        job_id,
        timestamp: brain_domain::jobs::JobTimestamp(1000),
    });
    event_log
        .append(&EventEnvelope::new("scheduler".to_string(), ev2))
        .unwrap(); // Seq 2

    // 3. JobProgressed
    let ev3 = DomainEvent::Core(brain_domain::DomainEvent::JobProgressed {
        job_id,
        progress: brain_domain::jobs::JobProgress::Determinate {
            completed: 5,
            total: 10,
            unit: brain_domain::jobs::ProgressUnit::Files,
        },
    });
    event_log
        .append(&EventEnvelope::new("scheduler".to_string(), ev3))
        .unwrap(); // Seq 3

    // 4. JobCompleted
    let ev4 = DomainEvent::Core(brain_domain::DomainEvent::JobCompleted {
        job_id,
        timestamp: brain_domain::jobs::JobTimestamp(2000),
    });
    event_log
        .append(&EventEnvelope::new("scheduler".to_string(), ev4))
        .unwrap(); // Seq 4

    // Run catch-up
    runner.catch_up().unwrap();

    // Invariant 1: Parity with aggregate state
    let read_model = job_repo.find_by_id(&job_id.0).unwrap().unwrap();
    assert_eq!(read_model.job_id, job_id.0);
    assert_eq!(read_model.kind, "indexing");
    assert_eq!(read_model.owner, "system");
    assert_eq!(read_model.state, "completed");
    assert_eq!(read_model.priority, 1); // High priority = 1
    assert_eq!(read_model.progress, 100); // completed progress is set to 100%
    assert_eq!(read_model.started_at, Some(1000));
    assert_eq!(read_model.completed_at, Some(2000));
    assert_eq!(read_model.updated_sequence, 4);

    // Invariant 2: Replay Determinism
    // Reset reading, clear read model table, rebuild
    runner.rebuild_projection(ProjectionId::Jobs).unwrap();

    let replayed_model = job_repo.find_by_id(&job_id.0).unwrap().unwrap();
    assert_eq!(replayed_model, read_model); // Byte-for-byte matching state!

    // Invariant 3: Arbitrary Interruption & Recovery
    // Clear read model and reset checkpoint manually to simulate initial state
    job_repo.clear_all().unwrap();
    checkpoint_repo.save_checkpoint("jobs", 0).unwrap();

    // Process events 1 and 2 manually (simulating crash before event 3 was reduced/checkpointed)
    let raw_events = event_log.read_from(1, 10).unwrap();
    assert_eq!(raw_events.len(), 4);

    let conn = pool.get().unwrap();
    let reducer = JobProjectionReducer::new(job_repo.clone());
    reducer.reduce(&conn, &raw_events[0]).unwrap();
    reducer.reduce(&conn, &raw_events[1]).unwrap();
    checkpoint_repo.save_checkpoint("jobs", 2).unwrap(); // Checkpoint is at 2

    let intermediate_model = job_repo.find_by_id(&job_id.0).unwrap().unwrap();
    assert_eq!(intermediate_model.state, "running");
    assert_eq!(intermediate_model.progress, 0);

    // Resume from interrupted state by running regular catch-up through runner.
    // The runner should read starting at sequence 3, reducing event 3 and 4.
    runner.catch_up().unwrap();

    let final_model = job_repo.find_by_id(&job_id.0).unwrap().unwrap();
    assert_eq!(final_model, read_model);
}

#[test]
fn test_session_projection_parity_rebuild_and_interruption() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool.clone()));
    let runner = ProjectionRunner::new(
        event_log.clone(),
        checkpoint_repo.clone(),
        Arc::new(ProjectionNotificationBus::new()),
    );

    let session_repo = Arc::new(SqliteSessionReadModelRepository::new(pool.clone()));
    let reducer = SessionProjectionReducer::new(session_repo.clone());
    runner.register(Arc::new(reducer)).unwrap();

    let session_id = brain_domain::SessionId::new();

    // 1. SessionCreated
    let ev1 = DomainEvent::Core(brain_domain::DomainEvent::SessionCreated {
        session_id,
        title: brain_domain::SessionTitle("Initial Title".to_string()),
        created_at: brain_domain::SessionTimestamp(100),
    });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev1))
        .unwrap(); // Seq 1

    // 2. SessionRenamed
    let ev2 = DomainEvent::Core(brain_domain::DomainEvent::SessionRenamed {
        session_id,
        title: brain_domain::SessionTitle("Updated Title".to_string()),
        updated_at: brain_domain::SessionTimestamp(150),
    });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev2))
        .unwrap(); // Seq 2

    // 3. SessionPinnedChanged (true)
    let ev3 = DomainEvent::Core(brain_domain::DomainEvent::SessionPinnedChanged {
        session_id,
        pinned: true,
        updated_at: brain_domain::SessionTimestamp(200),
    });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev3))
        .unwrap(); // Seq 3

    // 4. SessionArchived
    let ev4 = DomainEvent::Core(brain_domain::DomainEvent::SessionArchived {
        session_id,
        updated_at: brain_domain::SessionTimestamp(250),
    });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev4))
        .unwrap(); // Seq 4

    // Run catch-up
    runner.catch_up().unwrap();

    // Invariant 1: Parity with aggregate state
    let read_model = session_repo.find_by_id(&session_id).unwrap().unwrap();
    assert_eq!(read_model.session_id, session_id);
    assert_eq!(read_model.title, "Updated Title");
    assert!(read_model.is_archived);
    assert!(read_model.is_pinned);
    assert_eq!(read_model.created_at, brain_domain::SessionTimestamp(100));
    assert_eq!(read_model.updated_at, brain_domain::SessionTimestamp(250));
    assert_eq!(read_model.updated_sequence, 4);

    // Invariant 2: Replay Determinism
    runner.rebuild_projection(ProjectionId::Sessions).unwrap();
    let replayed_model = session_repo.find_by_id(&session_id).unwrap().unwrap();
    assert_eq!(replayed_model, read_model);

    // Invariant 3: Arbitrary Interruption & Recovery
    session_repo.clear_all().unwrap();
    checkpoint_repo.save_checkpoint("sessions", 0).unwrap();

    let raw_events = event_log.read_from(1, 10).unwrap();
    assert_eq!(raw_events.len(), 4);

    let conn = pool.get().unwrap();
    let reducer = SessionProjectionReducer::new(session_repo.clone());
    reducer.reduce(&conn, &raw_events[0]).unwrap();
    reducer.reduce(&conn, &raw_events[1]).unwrap();
    checkpoint_repo.save_checkpoint("sessions", 2).unwrap();

    let intermediate_model = session_repo.find_by_id(&session_id).unwrap().unwrap();
    assert_eq!(intermediate_model.title, "Updated Title");
    assert!(!intermediate_model.is_pinned);
    assert!(!intermediate_model.is_archived);

    // Resume from interrupted state
    runner.catch_up().unwrap();
    let final_model = session_repo.find_by_id(&session_id).unwrap().unwrap();
    assert_eq!(final_model, read_model);

    // 5. SessionRestored
    let ev5 = DomainEvent::Core(brain_domain::DomainEvent::SessionRestored {
        session_id,
        updated_at: brain_domain::SessionTimestamp(300),
    });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev5))
        .unwrap(); // Seq 5
    runner.catch_up().unwrap();

    let restored_model = session_repo.find_by_id(&session_id).unwrap().unwrap();
    assert!(!restored_model.is_archived);
    assert_eq!(
        restored_model.updated_at,
        brain_domain::SessionTimestamp(300)
    );

    // 6. SessionDeleted
    let ev6 = DomainEvent::Core(brain_domain::DomainEvent::SessionDeleted { session_id });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev6))
        .unwrap(); // Seq 6
    runner.catch_up().unwrap();

    assert!(session_repo.find_by_id(&session_id).unwrap().is_none());
}

#[test]
fn test_search_projection() {
    use brain_domain::{
        MessageId, MessageRole, MessageSnapshot, MessageTimestamp, SearchDocumentKind,
    };
    use brain_services::SearchProjectionReducer;
    use brain_storage::{SearchQuery, SqliteSearchRepository};

    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool.clone()));
    let search_repo = Arc::new(SqliteSearchRepository::new(pool.clone()));

    let runner = ProjectionRunner::new(
        event_log.clone(),
        checkpoint_repo,
        Arc::new(ProjectionNotificationBus::new()),
    );
    let reducer = SearchProjectionReducer::new(search_repo.clone());
    runner.register(Arc::new(reducer)).unwrap();

    let session_id = brain_domain::SessionId::new();
    let title = brain_domain::SessionTitle("Initial Session Title".to_string());

    // 1. SessionCreated
    let ev1 = DomainEvent::Core(brain_domain::DomainEvent::SessionCreated {
        session_id,
        title: title.clone(),
        created_at: brain_domain::SessionTimestamp(100),
    });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev1))
        .unwrap(); // Seq 1

    // 2. MessageAdded A
    let msg1_id = MessageId::new();
    let ev2 = DomainEvent::Core(brain_domain::DomainEvent::MessageAdded {
        session_id,
        message: MessageSnapshot {
            id: msg1_id,
            role: MessageRole::User,
            content: "Hello from rust agent development".to_string(),
            timestamp: MessageTimestamp(120),
        },
    });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev2))
        .unwrap(); // Seq 2

    // 3. MessageAdded B
    let msg2_id = MessageId::new();
    let ev3 = DomainEvent::Core(brain_domain::DomainEvent::MessageAdded {
        session_id,
        message: MessageSnapshot {
            id: msg2_id,
            role: MessageRole::Assistant,
            content: "Indeed, indexing is fully rebuilt from event log".to_string(),
            timestamp: MessageTimestamp(130),
        },
    });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev3))
        .unwrap(); // Seq 3

    // 4. SessionRenamed
    let ev4 = DomainEvent::Core(brain_domain::DomainEvent::SessionRenamed {
        session_id,
        title: brain_domain::SessionTitle("Renamed Search Session".to_string()),
        updated_at: brain_domain::SessionTimestamp(150),
    });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev4))
        .unwrap(); // Seq 4

    // Run catch-up
    runner.catch_up().unwrap();

    // Verify search results
    let q1 = SearchQuery {
        text: "Initial".to_string(),
        kinds: None,
        limit: None,
        offset: None,
    };
    let res = search_repo.search(&q1).unwrap();
    // Should be empty since it was renamed
    assert!(res.is_empty());

    let q2 = SearchQuery {
        text: "Renamed".to_string(),
        kinds: None,
        limit: None,
        offset: None,
    };
    let res2 = search_repo.search(&q2).unwrap();
    assert_eq!(res2.len(), 1);
    assert_eq!(res2[0].title, "Renamed Search Session");
    assert_eq!(res2[0].kind, SearchDocumentKind::Session);

    // Verify message search
    let q3 = SearchQuery {
        text: "rust".to_string(),
        kinds: Some(vec![SearchDocumentKind::Message]),
        limit: None,
        offset: None,
    };
    let res3 = search_repo.search(&q3).unwrap();
    assert_eq!(res3.len(), 1);
    assert_eq!(res3[0].body, "Hello from rust agent development");

    // Invariant: Rebuild Parity
    runner.rebuild_projection(ProjectionId::Search).unwrap();
    let res2_rebuilt = search_repo.search(&q2).unwrap();
    assert_eq!(res2_rebuilt.len(), 1);
    assert_eq!(res2_rebuilt[0].title, "Renamed Search Session");

    // Invariant: Idempotency (reducing same event doesn't duplicate)
    let raw_events = event_log.read_from(1, 10).unwrap();
    let conn = pool.get().unwrap();
    let manual_reducer = SearchProjectionReducer::new(search_repo.clone());
    // Re-apply Seq 4
    manual_reducer.reduce(&conn, &raw_events[3]).unwrap();
    let res2_idempotent = search_repo.search(&q2).unwrap();
    assert_eq!(res2_idempotent.len(), 1);

    // Invariant: Deletion Replay
    let ev_del = DomainEvent::Core(brain_domain::DomainEvent::SessionDeleted { session_id });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev_del))
        .unwrap(); // Seq 5
    runner.catch_up().unwrap();

    // Verify session and all its messages are deleted from FTS5 index
    let q_all = SearchQuery {
        text: "development".to_string(),
        kinds: None,
        limit: None,
        offset: None,
    };
    assert!(search_repo.search(&q_all).unwrap().is_empty());
}
