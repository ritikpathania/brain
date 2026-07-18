use crate::projections::{ProjectionId, ProjectionNotification, ProjectionNotificationBus};
use crate::query::dto::{JobSummary, SearchSummary, SessionSummary};
use crate::query::filters::{JobQuery, SearchQuery, SessionQuery};
use crate::query::traits::{JobQueryService, SearchQueryService, SessionQueryService};
use brain_core::errors::BrainError;
use brain_events::SequenceNumber;
use brain_storage::SqliteProjectionCheckpointRepository;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;

/// Strongly-typed key representing query types and parameters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubscriptionKey {
    /// Subscription to sessions read model.
    Session(SessionQuery),
    /// Subscription to background jobs read model.
    Job(JobQuery),
    /// Subscription to search projection.
    Search(SearchQuery),
}

/// Fanned-out query response payload.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryResponse {
    /// Sessions results DTO.
    Session {
        /// Checkpoint sequence when query was executed.
        sequence: SequenceNumber,
        /// DTO value vector.
        value: Vec<SessionSummary>,
    },
    /// Background jobs results DTO.
    Job {
        /// Checkpoint sequence when query was executed.
        sequence: SequenceNumber,
        /// DTO value vector.
        value: Vec<JobSummary>,
    },
    /// Search FTS5 results DTO.
    Search {
        /// Checkpoint sequence when query was executed.
        sequence: SequenceNumber,
        /// DTO value vector.
        value: Vec<SearchSummary>,
    },
}

struct SubscriptionState {
    last_seen_sequence: SequenceNumber,
    latest_sequence: SequenceNumber,
    sender: watch::Sender<QueryResponse>,
}

/// Central registry supervising query dirty invalidations, coalescing, and subscriber fan-out.
pub struct QuerySubscriptionRegistry {
    session_service: Arc<dyn SessionQueryService>,
    job_service: Arc<dyn JobQueryService>,
    search_service: Arc<dyn SearchQueryService>,
    checkpoint_repo: Arc<SqliteProjectionCheckpointRepository>,
    states: Arc<Mutex<HashMap<SubscriptionKey, SubscriptionState>>>,
    invalidation_tx: mpsc::Sender<SubscriptionKey>,
}

impl QuerySubscriptionRegistry {
    /// Creates a new `QuerySubscriptionRegistry` and starts the debounced invalidation loop.
    pub fn new(
        session_service: Arc<dyn SessionQueryService>,
        job_service: Arc<dyn JobQueryService>,
        search_service: Arc<dyn SearchQueryService>,
        checkpoint_repo: Arc<SqliteProjectionCheckpointRepository>,
        notification_bus: Arc<ProjectionNotificationBus>,
    ) -> Arc<Self> {
        let (invalidation_tx, invalidation_rx) = mpsc::channel(100);
        let registry = Arc::new(Self {
            session_service,
            job_service,
            search_service,
            checkpoint_repo,
            states: Arc::new(Mutex::new(HashMap::new())),
            invalidation_tx,
        });

        // 1. Spawns the coalescing scheduler task
        let reg_clone = Arc::clone(&registry);
        tokio::spawn(async move {
            reg_clone.run_invalidation_loop(invalidation_rx).await;
        });

        // 2. Subscribe synchronously to notification bus before spawning listener task
        // to avoid missing early notifications published during catch up.
        let mut bus_rx = notification_bus.subscribe();
        let reg_clone2 = Arc::clone(&registry);
        tokio::spawn(async move {
            loop {
                match bus_rx.recv().await {
                    Ok(notification) => {
                        reg_clone2.handle_notification(notification).await;
                    }
                    Err(e) => {
                        if let tokio::sync::broadcast::error::RecvError::Closed = e {
                            break;
                        }
                    }
                }
            }
        });

        registry
    }

    /// Subscribes to query updates. Performs an initial query execution on first registration.
    pub fn subscribe(
        &self,
        key: SubscriptionKey,
    ) -> Result<watch::Receiver<QueryResponse>, BrainError> {
        let mut states = self.states.lock();
        if let Some(state) = states.get(&key) {
            return Ok(state.sender.subscribe());
        }

        // Run the query once immediately to get current state
        let initial_response = self.execute_query(&key)?;
        let (tx, rx) = watch::channel(initial_response);

        let initial_seq = self.get_latest_checkpoint(&key)?;

        states.insert(
            key,
            SubscriptionState {
                last_seen_sequence: initial_seq,
                latest_sequence: initial_seq,
                sender: tx,
            },
        );

        Ok(rx)
    }

    fn handle_notification(
        &self,
        notification: ProjectionNotification,
    ) -> impl std::future::Future<Output = ()> + Send {
        let mut states = self.states.lock();
        let mut to_invalidate = Vec::new();

        for (key, state) in states.iter_mut() {
            if key.matches_projection(notification.projection_id) {
                if notification.sequence > state.latest_sequence {
                    state.latest_sequence = notification.sequence;
                    to_invalidate.push(key.clone());
                }
            }
        }

        let tx = self.invalidation_tx.clone();
        async move {
            for key in to_invalidate {
                let _ = tx.send(key).await;
            }
        }
    }

    async fn run_invalidation_loop(&self, mut rx: mpsc::Receiver<SubscriptionKey>) {
        let mut pending = HashSet::new();

        while let Some(key) = rx.recv().await {
            pending.insert(key);

            // Wait 50ms to coalesce rapid invalidation events
            sleep(Duration::from_millis(50)).await;

            // Drains all messages currently queued up to coalesce them
            while let Ok(extra_key) = rx.try_recv() {
                pending.insert(extra_key);
            }

            for key in pending.drain() {
                let (should_run, seq) = {
                    let states = self.states.lock();
                    if let Some(state) = states.get(&key) {
                        (
                            state.latest_sequence > state.last_seen_sequence,
                            state.latest_sequence,
                        )
                    } else {
                        (false, SequenceNumber(0))
                    }
                };

                if should_run {
                    if let Ok(response) = self.execute_query(&key) {
                        let mut states = self.states.lock();
                        if let Some(state) = states.get_mut(&key) {
                            let _ = state.sender.send(response);
                            state.last_seen_sequence = seq;
                        }
                    }
                }
            }
        }
    }

    fn execute_query(&self, key: &SubscriptionKey) -> Result<QueryResponse, BrainError> {
        let seq = self.get_latest_checkpoint(key)?;
        match key {
            SubscriptionKey::Session(query) => {
                let res = self.session_service.list_sessions(query.clone())?;
                Ok(QueryResponse::Session {
                    sequence: seq,
                    value: res,
                })
            }
            SubscriptionKey::Job(query) => {
                let res = self.job_service.list_jobs(query.clone())?;
                Ok(QueryResponse::Job {
                    sequence: seq,
                    value: res,
                })
            }
            SubscriptionKey::Search(query) => {
                let res = self.search_service.search(query.clone())?;
                Ok(QueryResponse::Search {
                    sequence: seq,
                    value: res,
                })
            }
        }
    }

    /// Explicitly removes an active subscription and halts future updates.
    pub fn unsubscribe(&self, key: &SubscriptionKey) {
        let mut states = self.states.lock();
        states.remove(key);
    }

    /// Checks if a subscription key is active in the registry.
    pub fn has_subscription(&self, key: &SubscriptionKey) -> bool {
        let states = self.states.lock();
        states.contains_key(key)
    }

    fn get_latest_checkpoint(&self, key: &SubscriptionKey) -> Result<SequenceNumber, BrainError> {
        let db_name = match key {
            SubscriptionKey::Session(_) => "sessions",
            SubscriptionKey::Job(_) => "jobs",
            SubscriptionKey::Search(_) => "search",
        };
        let val = self.checkpoint_repo.get_checkpoint(db_name)?;
        Ok(SequenceNumber(val))
    }
}

impl SubscriptionKey {
    fn matches_projection(&self, projection_id: ProjectionId) -> bool {
        match (self, projection_id) {
            (SubscriptionKey::Session(_), ProjectionId::Sessions) => true,
            (SubscriptionKey::Job(_), ProjectionId::Jobs) => true,
            (SubscriptionKey::Search(_), ProjectionId::Search) => true,
            _ => false,
        }
    }
}
