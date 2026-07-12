use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::time::sleep;

use brain_core::repositories::SessionRepository;
use brain_domain::{SessionId, SessionTitle, SessionTimestamp};
use brain_events::{EventEnvelope, DomainEvent, SequenceNumber, EventLog};
use brain_storage::{
    TestStorage, SqliteJobReadModelRepository, SqliteSessionReadModelRepository,
    SqliteSearchRepository, SqliteEventLog, SqliteProjectionCheckpointRepository
};
use brain_services::SystemEventLog;
use brain_services::query::{
    SessionQuery, SessionQueryService,
    SqliteJobQueryService, SqliteSessionQueryService, SqliteSearchQueryService,
    SubscriptionKey, QuerySubscriptionRegistry,
    SessionSubscriptionService, SqliteSessionSubscriptionService
};
use brain_services::projections::{
    ProjectionRunner, SessionProjectionReducer,
    ProjectionNotificationBus
};

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
    fn list_sessions(&self, query: SessionQuery) -> Result<Vec<brain_services::query::dto::SessionSummary>, brain_core::errors::BrainError> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        self.inner.list_sessions(query)
    }

    fn get_session(&self, id: &SessionId) -> Result<Option<brain_services::query::dto::SessionDetails>, brain_core::errors::BrainError> {
        self.inner.get_session(id)
    }
}

#[tokio::test]
async fn test_live_subscription_stream_and_fan_out() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool.clone()));
    let notification_bus = Arc::new(ProjectionNotificationBus::new());

    let runner = ProjectionRunner::new(event_log.clone(), checkpoint_repo.clone(), Arc::clone(&notification_bus));
    let session_proj_repo = Arc::new(SqliteSessionReadModelRepository::new(pool.clone()));
    let reducer = SessionProjectionReducer::new(session_proj_repo.clone());
    runner.register(Box::new(reducer)).unwrap();

    let session_repo: Arc<dyn SessionRepository> = test_storage.store();
    let inner_service = SqliteSessionQueryService::new(session_proj_repo, session_repo);
    let execution_counter = Arc::new(AtomicUsize::new(0));
    let counting_service = Arc::new(CountingSessionQueryService::new(inner_service, Arc::clone(&execution_counter)));

    let job_service = Arc::new(SqliteJobQueryService::new(Arc::new(SqliteJobReadModelRepository::new(pool.clone()))));
    let search_service = Arc::new(SqliteSearchQueryService::new(Arc::new(SqliteSearchRepository::new(pool))));

    let registry = QuerySubscriptionRegistry::new(
        counting_service,
        job_service,
        search_service,
        checkpoint_repo,
        Arc::clone(&notification_bus),
    );

    let sub_service = SqliteSessionSubscriptionService::new(registry);

    // Subscribe client 1 & client 2
    let mut live_query1 = sub_service.subscribe(SessionQuery::default()).unwrap();
    let mut live_query2 = sub_service.subscribe(SessionQuery::default()).unwrap();

    // Verify initial DTO snapshots
    let snap1 = live_query1.snapshot();
    let snap2 = live_query2.snapshot();
    assert_eq!(snap1.sequence(), SequenceNumber(0));
    assert_eq!(snap1.value().len(), 0);
    assert_eq!(snap2.value().len(), 0);

    // 2. Publish a SessionCreated event to event log and catch up
    let session_id = SessionId::new();
    let ev = DomainEvent::Core(brain_domain::DomainEvent::SessionCreated {
        session_id,
        title: SessionTitle("Live Session".to_string()),
        created_at: SessionTimestamp(200),
    });
    event_log.append(&EventEnvelope::new("session_service".to_string(), ev)).unwrap();

    // Catch up runner
    runner.catch_up().unwrap();

    // Wait for the debounced invalidation to run
    sleep(Duration::from_millis(150)).await;

    // Verify next() blocks and retrieves the updated snapshot on both clients
    let next_snap1 = live_query1.next().await.unwrap();
    let next_snap2 = live_query2.next().await.unwrap();

    assert_eq!(next_snap1.sequence(), SequenceNumber(1));
    assert_eq!(next_snap1.value().len(), 1);
    assert_eq!(next_snap1.value()[0].title.0, "Live Session");
    assert_eq!(next_snap2.value().len(), 1);
}

#[tokio::test]
async fn test_deregistration_on_drop_cancellation() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool.clone()));
    let notification_bus = Arc::new(ProjectionNotificationBus::new());

    let runner = ProjectionRunner::new(event_log.clone(), checkpoint_repo.clone(), Arc::clone(&notification_bus));
    let session_proj_repo = Arc::new(SqliteSessionReadModelRepository::new(pool.clone()));
    let reducer = SessionProjectionReducer::new(session_proj_repo.clone());
    runner.register(Box::new(reducer)).unwrap();

    let session_repo: Arc<dyn SessionRepository> = test_storage.store();
    let inner_service = SqliteSessionQueryService::new(session_proj_repo, session_repo);
    let execution_counter = Arc::new(AtomicUsize::new(0));
    let counting_service = Arc::new(CountingSessionQueryService::new(inner_service, Arc::clone(&execution_counter)));

    let job_service = Arc::new(SqliteJobQueryService::new(Arc::new(SqliteJobReadModelRepository::new(pool.clone()))));
    let search_service = Arc::new(SqliteSearchQueryService::new(Arc::new(SqliteSearchRepository::new(pool))));

    let registry = QuerySubscriptionRegistry::new(
        counting_service,
        job_service,
        search_service,
        checkpoint_repo,
        Arc::clone(&notification_bus),
    );

    let sub_service = SqliteSessionSubscriptionService::new(Arc::clone(&registry));
    let query = SessionQuery::default();
    let key = SubscriptionKey::Session(query.clone());

    // Subscribe
    let live_query = sub_service.subscribe(query).unwrap();

    // Verify registry registered it
    assert!(registry.has_subscription(&key));

    // Drop the LiveQuery lifecycle handle
    drop(live_query);

    // Verify registry automatically deregistered it
    assert!(!registry.has_subscription(&key));

    // Publish event and catch up
    let session_id = SessionId::new();
    let ev = DomainEvent::Core(brain_domain::DomainEvent::SessionCreated {
        session_id,
        title: SessionTitle("Post Drop Session".to_string()),
        created_at: SessionTimestamp(300),
    });
    event_log.append(&EventEnvelope::new("session_service".to_string(), ev)).unwrap();
    runner.catch_up().unwrap();

    // Wait and verify no extra query execution occurred
    sleep(Duration::from_millis(150)).await;
    // Initial was 1, no second query execution occurred because subscription was cancelled on drop!
    assert_eq!(execution_counter.load(Ordering::SeqCst), 1);
}
