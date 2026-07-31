use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use brain_core::repositories::SessionRepository;
use brain_domain::{SessionId, SessionTimestamp, SessionTitle};
use brain_events::{DomainEvent, EventEnvelope, EventLog, SequenceNumber};
use brain_services::projections::{
    ProjectionId, ProjectionNotification, ProjectionNotificationBus, ProjectionRunner,
    SessionProjectionReducer,
};
use brain_services::query::{
    JobQueryService, QueryResponse, QuerySubscriptionRegistry, SearchQueryService, SessionQuery,
    SessionQueryService, SqliteJobQueryService, SqliteSearchQueryService,
    SqliteSessionQueryService, SubscriptionKey,
};
use brain_services::SystemEventLog;
use brain_storage::{
    SqliteEventLog, SqliteJobReadModelRepository, SqliteProjectionCheckpointRepository,
    SqliteSearchRepository, SqliteSessionReadModelRepository, TestStorage,
};

// A wrapping wrapper around SessionQueryService to count executions
struct CountingSessionQueryService {
    inner: SqliteSessionQueryService,
    counter: Arc<AtomicUsize>,
}

impl CountingSessionQueryService {
    fn new(inner: SqliteSessionQueryService, counter: Arc<AtomicUsize>) -> Self {
        Self { inner, counter }
    }
}

impl SessionQueryService for CountingSessionQueryService {
    fn list_sessions(
        &self,
        query: SessionQuery,
    ) -> Result<Vec<brain_services::query::dto::SessionSummary>, brain_core::errors::BrainError>
    {
        self.counter.fetch_add(1, Ordering::SeqCst);
        self.inner.list_sessions(query)
    }

    fn get_session(
        &self,
        id: &SessionId,
    ) -> Result<Option<brain_services::query::dto::SessionDetails>, brain_core::errors::BrainError>
    {
        self.inner.get_session(id)
    }
}

#[tokio::test]
async fn test_notification_broadcast() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool.clone()));

    let notification_bus = Arc::new(ProjectionNotificationBus::new());
    let mut rx = notification_bus.subscribe();

    let runner = ProjectionRunner::new(
        event_log.clone(),
        checkpoint_repo,
        Arc::clone(&notification_bus),
    );
    let session_proj_repo = Arc::new(SqliteSessionReadModelRepository::new(pool));
    let reducer = SessionProjectionReducer::new(session_proj_repo);
    runner.register(Arc::new(reducer)).unwrap();

    // Publish a SessionCreated event
    let session_id = SessionId::new();
    let ev = DomainEvent::Core(brain_domain::DomainEvent::SessionCreated {
        session_id,
        title: SessionTitle("Initial Title".to_string()),
        created_at: SessionTimestamp(100),
    });
    event_log
        .append(&EventEnvelope::new("session_service".to_string(), ev))
        .unwrap(); // Seq 1

    // Catch up
    runner.catch_up().unwrap();

    // Verify a notification was published to the bus indicating sequence 1
    let notification = rx.recv().await.unwrap();
    assert_eq!(notification.projection_id, ProjectionId::Sessions);
    assert_eq!(notification.sequence, SequenceNumber(1));
}

#[tokio::test]
async fn test_coalesced_invalidation_and_deduplication() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool.clone()));
    let notification_bus = Arc::new(ProjectionNotificationBus::new());

    let runner = ProjectionRunner::new(
        event_log.clone(),
        checkpoint_repo.clone(),
        Arc::clone(&notification_bus),
    );
    let session_proj_repo = Arc::new(SqliteSessionReadModelRepository::new(pool.clone()));
    let reducer = SessionProjectionReducer::new(session_proj_repo.clone());
    runner.register(Arc::new(reducer)).unwrap();

    // Create counting wrapper
    let session_repo: Arc<dyn SessionRepository> = test_storage.store();
    let inner_service = SqliteSessionQueryService::new(session_proj_repo, session_repo);
    let execution_counter = Arc::new(AtomicUsize::new(0));
    let counting_service = Arc::new(CountingSessionQueryService::new(
        inner_service,
        Arc::clone(&execution_counter),
    ));

    let job_service = Arc::new(SqliteJobQueryService::new(Arc::new(
        SqliteJobReadModelRepository::new(pool.clone()),
    )));
    let search_service = Arc::new(SqliteSearchQueryService::new(Arc::new(
        SqliteSearchRepository::new(pool),
    )));

    let registry = QuerySubscriptionRegistry::new(
        counting_service,
        job_service,
        search_service,
        checkpoint_repo,
        Arc::clone(&notification_bus),
    );

    let query_key = SubscriptionKey::Session(SessionQuery::default());

    // 1. First subscriber
    let mut rx1 = registry.subscribe(query_key.clone()).unwrap();
    // 2. Second subscriber (concurrent)
    let mut rx2 = registry.subscribe(query_key.clone()).unwrap();

    // Initial subscription runs the query once
    assert_eq!(execution_counter.load(Ordering::SeqCst), 1);

    // Drain initial DTOs
    let res1 = rx1.borrow().clone();
    let res2 = rx2.borrow().clone();
    assert!(matches!(res1, QueryResponse::Session { .. }));
    assert_eq!(res1, res2);

    // 3. Trigger 5 notifications in rapid succession
    for i in 2..=6 {
        notification_bus.publish(ProjectionNotification {
            projection_id: ProjectionId::Sessions,
            sequence: SequenceNumber(i),
        });
    }

    // Wait for the 50ms debounce window to expire + extra margin
    sleep(Duration::from_millis(150)).await;

    // Verify only ONE query execution happened for all 5 notifications combined
    assert_eq!(execution_counter.load(Ordering::SeqCst), 2);

    // Verify both subscribers received exactly one update
    rx1.changed().await.unwrap();
    let update1 = rx1.borrow().clone();
    rx2.changed().await.unwrap();
    let update2 = rx2.borrow().clone();
    assert_eq!(update1, update2);
}

#[tokio::test]
async fn test_graceful_shutdown_and_restart_recovery() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool.clone()));
    let notification_bus = Arc::new(ProjectionNotificationBus::new());

    let session_proj_repo = Arc::new(SqliteSessionReadModelRepository::new(pool.clone()));
    let session_repo: Arc<dyn SessionRepository> = test_storage.store();
    let session_service = Arc::new(SqliteSessionQueryService::new(
        session_proj_repo,
        session_repo,
    ));
    let job_service = Arc::new(SqliteJobQueryService::new(Arc::new(
        SqliteJobReadModelRepository::new(pool.clone()),
    )));
    let search_service = Arc::new(SqliteSearchQueryService::new(Arc::new(
        SqliteSearchRepository::new(pool.clone()),
    )));

    // Simulate 3 events and advance projection checkpoint to 3
    checkpoint_repo.save_checkpoint("sessions", 3).unwrap();

    // Start Registry 1
    let registry1 = QuerySubscriptionRegistry::new(
        Arc::clone(&session_service) as Arc<dyn SessionQueryService>,
        Arc::clone(&job_service) as Arc<dyn JobQueryService>,
        Arc::clone(&search_service) as Arc<dyn SearchQueryService>,
        Arc::clone(&checkpoint_repo),
        Arc::clone(&notification_bus),
    );

    let query_key = SubscriptionKey::Session(SessionQuery::default());
    let rx = registry1.subscribe(query_key.clone()).unwrap();
    let res = rx.borrow().clone();
    assert!(matches!(res, QueryResponse::Session { .. }));

    // Now registry1 "shuts down" (it falls out of scope, but we instantiate registry2 to simulate restart recovery)
    drop(registry1);

    // Registry2 starts with no past notifications replayed
    let registry2 = QuerySubscriptionRegistry::new(
        session_service,
        job_service,
        search_service,
        checkpoint_repo,
        notification_bus,
    );

    // Subscriber subscribing to Registry2 should immediately get correct current state from the persistent checkpoint
    let rx2 = registry2.subscribe(query_key).unwrap();
    let res2 = rx2.borrow().clone();
    assert!(matches!(res2, QueryResponse::Session { .. }));
}
