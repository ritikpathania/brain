use std::sync::Arc;
use uuid::Uuid;
use brain_core::repositories::SessionRepository;
use brain_domain::jobs::{JobId, JobOwner, JobState, JobTimestamp};
use brain_domain::{
    Session, SessionId, SessionTitle, SessionTimestamp, Message, MessageId, MessageRole,
    SearchDocument, SearchDocumentId, SearchDocumentKind, SearchMetadata
};
use brain_storage::{
    TestStorage, SqliteJobReadModelRepository, SqliteSessionReadModelRepository,
    SqliteSearchRepository, JobReadModel, SessionReadModel, SqliteEventLog, SqliteProjectionCheckpointRepository,
    ReadModelRepository
};
use brain_events::{EventEnvelope, DomainEvent, EventLog};
use brain_services::SystemEventLog;
use brain_services::query::{
    JobQuery, SessionQuery, SearchQuery, PaginationSpec, JobQueryService, SessionQueryService, SearchQueryService,
    SqliteJobQueryService, SqliteSessionQueryService, SqliteSearchQueryService
};
use brain_services::projections::{
    ProjectionRunner, SessionProjectionReducer, SearchProjectionReducer,
    ProjectionNotificationBus
};

#[test]
fn test_job_query_service() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let repo = Arc::new(SqliteJobReadModelRepository::new(pool));
    let service = SqliteJobQueryService::new(repo.clone());

    let job_id1 = Uuid::new_v4();
    let job_id2 = Uuid::new_v4();

    let job1 = JobReadModel {
        job_id: job_id1,
        kind: "tool".to_string(),
        owner: "system".to_string(),
        state: "pending".to_string(),
        priority: 2, // Normal
        progress: 0,
        started_at: None,
        completed_at: None,
        failure_reason: None,
        updated_sequence: 1,
    };

    let job2 = JobReadModel {
        job_id: job_id2,
        kind: "retrieval".to_string(),
        owner: "user:ritik".to_string(),
        state: "completed".to_string(),
        priority: 1, // High
        progress: 100,
        started_at: Some(1700000000),
        completed_at: Some(1700000100),
        failure_reason: None,
        updated_sequence: 2,
    };

    repo.save(&job1).unwrap();
    repo.save(&job2).unwrap();

    // Query list
    let list_all = service.list_jobs(JobQuery::default()).unwrap();
    assert_eq!(list_all.len(), 2);
    // Order is updated_sequence DESC
    assert_eq!(list_all[0].job_id, JobId(job_id2));
    assert_eq!(list_all[1].job_id, JobId(job_id1));

    // Query filtered
    let query_filtered = service.list_jobs(JobQuery {
        owner: Some(JobOwner::System),
        state: Some(JobState::Pending),
        pagination: None,
    }).unwrap();
    assert_eq!(query_filtered.len(), 1);
    assert_eq!(query_filtered[0].job_id, JobId(job_id1));

    // Pagination limit
    let paginated = service.list_jobs(JobQuery {
        owner: None,
        state: None,
        pagination: Some(PaginationSpec {
            limit: Some(1),
            offset: None,
        }),
    }).unwrap();
    assert_eq!(paginated.len(), 1);
    assert_eq!(paginated[0].job_id, JobId(job_id2));

    // Get details
    let details = service.get_job(&JobId(job_id2)).unwrap().unwrap();
    assert_eq!(details.job_id, JobId(job_id2));
    assert_eq!(details.state, JobState::Completed);
    assert_eq!(details.started_at, Some(JobTimestamp(1700000000)));
    assert_eq!(details.completed_at, Some(JobTimestamp(1700000100)));
}

#[test]
fn test_session_query_service() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let proj_repo = Arc::new(SqliteSessionReadModelRepository::new(pool));
    
    // Core session repository interface is implemented by SqliteStorage
    let session_repo: Arc<dyn SessionRepository> = test_storage.store();
    let service = SqliteSessionQueryService::new(proj_repo.clone(), session_repo.clone());

    let session_id1 = SessionId::new();
    let session_id2 = SessionId::new();

    let s1_read = SessionReadModel {
        session_id: session_id1,
        title: "Session 1".to_string(),
        is_archived: false,
        is_pinned: true,
        created_at: SessionTimestamp(1000),
        updated_at: SessionTimestamp(1000),
        updated_sequence: 1,
    };

    let s2_read = SessionReadModel {
        session_id: session_id2,
        title: "Session 2".to_string(),
        is_archived: true,
        is_pinned: false,
        created_at: SessionTimestamp(2000),
        updated_at: SessionTimestamp(2000),
        updated_sequence: 2,
    };

    proj_repo.save(&s1_read).unwrap();
    proj_repo.save(&s2_read).unwrap();

    // Create a real session aggregate to load messages from
    let mut session = Session::new(session_id1, SessionTitle("Session 1".to_string()), SessionTimestamp(1000));
    let msg1 = Message::new(MessageId::new(), MessageRole::User, "Hello user query service".to_string());
    session.add_message(msg1).unwrap();
    session_repo.save_session(&session_id1, &session).unwrap();

    // Query active list
    let list_active = service.list_sessions(SessionQuery {
        is_archived: Some(false),
        is_pinned: None,
        pagination: None,
    }).unwrap();
    assert_eq!(list_active.len(), 1);
    assert_eq!(list_active[0].session_id, session_id1);

    // Get details with messages populated
    let details = service.get_session(&session_id1).unwrap().unwrap();
    assert_eq!(details.session_id, session_id1);
    assert_eq!(details.title.0, "Session 1");
    assert_eq!(details.messages.len(), 1);
    assert_eq!(details.messages[0].content, "Hello user query service");
}

#[test]
fn test_search_query_service() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let search_repo = Arc::new(SqliteSearchRepository::new(pool));
    let service = SqliteSearchQueryService::new(search_repo.clone());

    let doc_id = SearchDocumentId::new("session:1".to_string());
    let doc = SearchDocument {
        id: doc_id.clone(),
        kind: SearchDocumentKind::Session,
        title: "Introduction to Rust".to_string(),
        body: "Let us discuss zero cost abstractions".to_string(),
        metadata: SearchMetadata::Session {
            archived: false,
            pinned: true,
        },
    };

    search_repo.save(&doc, 1).unwrap();

    // Search query match
    let query = SearchQuery {
        text: "Rust zero".to_string(),
        kinds: None,
        pagination: None,
    };
    let results = service.search(query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, doc_id);
    assert_eq!(results[0].title, "Introduction to Rust");
    assert_eq!(results[0].metadata, SearchMetadata::Session { archived: false, pinned: true });
}

#[test]
fn test_cross_projection_consistency() {
    let test_storage = TestStorage::new();
    let pool = test_storage.store().pool().clone();
    let raw_log = Arc::new(SqliteEventLog::new(pool.clone()));
    let event_log = Arc::new(SystemEventLog::new(raw_log));
    let checkpoint_repo = Arc::new(SqliteProjectionCheckpointRepository::new(pool.clone()));

    let session_proj_repo = Arc::new(SqliteSessionReadModelRepository::new(pool.clone()));
    let search_repo = Arc::new(SqliteSearchRepository::new(pool));

    let runner = ProjectionRunner::new(event_log.clone(), checkpoint_repo, Arc::new(ProjectionNotificationBus::new()));

    let session_reducer = SessionProjectionReducer::new(session_proj_repo.clone());
    let search_reducer = SearchProjectionReducer::new(search_repo.clone());

    runner.register(Box::new(session_reducer)).unwrap();
    runner.register(Box::new(search_reducer)).unwrap();

    let session_repo: Arc<dyn SessionRepository> = test_storage.store();
    let session_query_service = SqliteSessionQueryService::new(session_proj_repo, session_repo);
    let search_query_service = SqliteSearchQueryService::new(search_repo);

    let session_id = SessionId::new();

    // 1. SessionCreated
    let ev1 = DomainEvent::Core(brain_domain::DomainEvent::SessionCreated {
        session_id,
        title: SessionTitle("Old Session Name".to_string()),
        created_at: SessionTimestamp(100),
    });
    event_log.append(&EventEnvelope::new("test".to_string(), ev1)).unwrap();

    runner.catch_up().unwrap();

    // 2. SessionRenamed
    let ev2 = DomainEvent::Core(brain_domain::DomainEvent::SessionRenamed {
        session_id,
        title: SessionTitle("New Updated Title".to_string()),
        updated_at: SessionTimestamp(150),
    });
    event_log.append(&EventEnvelope::new("test".to_string(), ev2)).unwrap();

    runner.catch_up().unwrap();

    // Verify SessionQueryService returns new title
    let session_details = session_query_service.get_session(&session_id).unwrap().unwrap();
    assert_eq!(session_details.title.0, "New Updated Title");

    // Verify SearchQueryService finds the new title
    let search_res = search_query_service.search(SearchQuery {
        text: "New Updated".to_string(),
        kinds: None,
        pagination: None,
    }).unwrap();
    assert_eq!(search_res.len(), 1);
    assert_eq!(search_res[0].title, "New Updated Title");

    // Verify SearchQueryService does NOT find the old title
    let search_old_res = search_query_service.search(SearchQuery {
        text: "Old Session".to_string(),
        kinds: None,
        pagination: None,
    }).unwrap();
    assert_eq!(search_old_res.len(), 0);
}
