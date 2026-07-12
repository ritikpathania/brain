use std::sync::Arc;
use brain_core::errors::BrainError;
use brain_events::SequenceNumber;
use crate::query::dto::{SessionSummary, JobSummary, SearchSummary};
use crate::query::filters::{SessionQuery, JobQuery, SearchQuery};
use crate::query::registry::{SubscriptionKey, QueryResponse, QuerySubscriptionRegistry};
use tokio::sync::watch;

/// Immutable query snapshot.
///
/// **Subscription Consistency Invariant**:
/// Each `QuerySnapshot` represents a complete, internally consistent execution of its query at a projection
/// checkpoint greater than or equal to the returned `SequenceNumber`. Intermediate projection updates may
/// be coalesced, but snapshots are never partially updated.
pub struct QuerySnapshot<T> {
    sequence: SequenceNumber,
    value: Arc<T>,
}

impl<T> QuerySnapshot<T> {
    /// Creates a new immutable `QuerySnapshot`.
    pub fn new(sequence: SequenceNumber, value: Arc<T>) -> Self {
        Self { sequence, value }
    }

    /// Accessor for the sequence checkpoint.
    pub fn sequence(&self) -> SequenceNumber {
        self.sequence
    }

    /// Accessor for the DTO value.
    pub fn value(&self) -> &Arc<T> {
        &self.value
    }
}

/// A transport-agnostic handle to a live updating query stream.
/// Dropping it cancels the subscription and releases all registry resources immediately.
#[async_trait::async_trait]
pub trait LiveQuery<T>: Send + Sync {
    /// Retrieves the current snapshot of the read model along with its sequence checkpoint.
    fn snapshot(&self) -> QuerySnapshot<T>;

    /// Blocks until the next snapshot update is available. Returns None if closed.
    async fn next(&mut self) -> Option<QuerySnapshot<T>>;
}

/// In-memory implementation of `LiveQuery` backing by tokio `watch::Receiver`.
pub struct WatchLiveQuery<T> {
    registry: Arc<QuerySubscriptionRegistry>,
    key: SubscriptionKey,
    receiver: watch::Receiver<QueryResponse>,
    extractor: Box<dyn Fn(QueryResponse) -> QuerySnapshot<T> + Send + Sync>,
}

#[async_trait::async_trait]
impl<T: Send + Sync> LiveQuery<T> for WatchLiveQuery<T> {
    fn snapshot(&self) -> QuerySnapshot<T> {
        let response = self.receiver.borrow().clone();
        (self.extractor)(response)
    }

    async fn next(&mut self) -> Option<QuerySnapshot<T>> {
        self.receiver.changed().await.ok()?;
        let response = self.receiver.borrow().clone();
        Some((self.extractor)(response))
    }
}

impl<T> Drop for WatchLiveQuery<T> {
    fn drop(&mut self) {
        self.registry.unsubscribe(&self.key);
    }
}

/// Service providing live query subscription streams for session summaries.
pub trait SessionSubscriptionService: Send + Sync {
    /// Subscribes to a stream of active session summaries matching the given filters.
    fn subscribe(&self, query: SessionQuery) -> Result<Box<dyn LiveQuery<Vec<SessionSummary>>>, BrainError>;
}

/// Service providing live query subscription streams for background job summaries.
pub trait JobSubscriptionService: Send + Sync {
    /// Subscribes to a stream of background job summaries matching the given filters.
    fn subscribe(&self, query: JobQuery) -> Result<Box<dyn LiveQuery<Vec<JobSummary>>>, BrainError>;
}

/// Service providing live query subscription streams for text search results.
pub trait SearchSubscriptionService: Send + Sync {
    /// Subscribes to a stream of search document matches.
    fn subscribe(&self, query: SearchQuery) -> Result<Box<dyn LiveQuery<Vec<SearchSummary>>>, BrainError>;
}

/// Concrete implementation of `SessionSubscriptionService`.
pub struct SqliteSessionSubscriptionService {
    registry: Arc<QuerySubscriptionRegistry>,
}

impl SqliteSessionSubscriptionService {
    /// Creates a new `SqliteSessionSubscriptionService`.
    pub fn new(registry: Arc<QuerySubscriptionRegistry>) -> Self {
        Self { registry }
    }
}

impl SessionSubscriptionService for SqliteSessionSubscriptionService {
    fn subscribe(&self, query: SessionQuery) -> Result<Box<dyn LiveQuery<Vec<SessionSummary>>>, BrainError> {
        let key = SubscriptionKey::Session(query);
        let rx = self.registry.subscribe(key.clone())?;

        let extractor = Box::new(|res| match res {
            QueryResponse::Session { sequence, value } => QuerySnapshot::new(sequence, Arc::new(value)),
            _ => panic!("Expected Session QueryResponse"),
        });

        Ok(Box::new(WatchLiveQuery {
            registry: Arc::clone(&self.registry),
            key,
            receiver: rx,
            extractor,
        }))
    }
}

/// Concrete implementation of `JobSubscriptionService`.
pub struct SqliteJobSubscriptionService {
    registry: Arc<QuerySubscriptionRegistry>,
}

impl SqliteJobSubscriptionService {
    /// Creates a new `SqliteJobSubscriptionService`.
    pub fn new(registry: Arc<QuerySubscriptionRegistry>) -> Self {
        Self { registry }
    }
}

impl JobSubscriptionService for SqliteJobSubscriptionService {
    fn subscribe(&self, query: JobQuery) -> Result<Box<dyn LiveQuery<Vec<JobSummary>>>, BrainError> {
        let key = SubscriptionKey::Job(query);
        let rx = self.registry.subscribe(key.clone())?;

        let extractor = Box::new(|res| match res {
            QueryResponse::Job { sequence, value } => QuerySnapshot::new(sequence, Arc::new(value)),
            _ => panic!("Expected Job QueryResponse"),
        });

        Ok(Box::new(WatchLiveQuery {
            registry: Arc::clone(&self.registry),
            key,
            receiver: rx,
            extractor,
        }))
    }
}

/// Concrete implementation of `SearchSubscriptionService`.
pub struct SqliteSearchSubscriptionService {
    registry: Arc<QuerySubscriptionRegistry>,
}

impl SqliteSearchSubscriptionService {
    /// Creates a new `SqliteSearchSubscriptionService`.
    pub fn new(registry: Arc<QuerySubscriptionRegistry>) -> Self {
        Self { registry }
    }
}

impl SearchSubscriptionService for SqliteSearchSubscriptionService {
    fn subscribe(&self, query: SearchQuery) -> Result<Box<dyn LiveQuery<Vec<SearchSummary>>>, BrainError> {
        let key = SubscriptionKey::Search(query);
        let rx = self.registry.subscribe(key.clone())?;

        let extractor = Box::new(|res| match res {
            QueryResponse::Search { sequence, value } => QuerySnapshot::new(sequence, Arc::new(value)),
            _ => panic!("Expected Search QueryResponse"),
        });

        Ok(Box::new(WatchLiveQuery {
            registry: Arc::clone(&self.registry),
            key,
            receiver: rx,
            extractor,
        }))
    }
}
