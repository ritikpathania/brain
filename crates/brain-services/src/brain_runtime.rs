use crate::query::SearchQuery;
use crate::{
    InMemoryEventDispatcher, SqliteCanonicalizer, SqliteProjectionManager, SqliteReflectionEngine,
};
use brain_core::{
    errors::BrainError,
    events::{CorrelationId, RuntimeEvent, RuntimeEventDispatcher},
    evolution::{Canonicalizer, Observation, StageTimings},
    projection::{ProjectionQuery, Projector},
    repositories::Storage,
};
use brain_domain::{EpochId, NodeId, SearchDocument};
use brain_observability::{timeline::OperationSpan, CorrelationIndex, ObservabilitySubscriber};
use brain_storage::{SqliteSearchRepository, SqliteStorage};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

// Compile-time proof that BrainRuntime is safe to share across threads.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BrainRuntime>();
};

/// Represents the execution state of a runtime capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityState {
    /// The capability is fully operational and healthy.
    Active,
    /// The capability is operational but executing in a degraded state.
    Degraded,
    /// The capability is offline or disabled.
    Inactive,
}

/// Metadata description of a supported capability within the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    /// Unique identifier name of the capability (e.g., "storage").
    pub name: &'static str,
    /// Schema or protocol version of the capability interface.
    pub version: u32,
    /// Human-readable explanation of what the capability does.
    pub description: &'static str,
    /// Current operational state of the capability.
    pub state: CapabilityState,
    /// Whether this capability is currently enabled.
    pub is_enabled: bool,
    /// Whether this capability is experimental and subject to change.
    pub is_experimental: bool,
}

/// Descriptive registry of runtime capabilities, immutable after startup.
#[derive(Debug, Clone)]
pub struct CapabilityRegistry {
    descriptors: Vec<CapabilityDescriptor>,
}

impl CapabilityRegistry {
    /// Creates a new empty capability registry.
    fn new() -> Self {
        Self {
            descriptors: Vec::new(),
        }
    }

    /// Registers a capability descriptor. Returns an error if the capability name is a duplicate.
    /// Private to crate to enforce post-startup immutability.
    fn register(&mut self, descriptor: CapabilityDescriptor) -> Result<(), BrainError> {
        if self.descriptors.iter().any(|d| d.name == descriptor.name) {
            return Err(BrainError::InvalidTransition {
                message: format!("Capability '{}' is already registered", descriptor.name),
            });
        }
        self.descriptors.push(descriptor);
        Ok(())
    }

    /// Lists all capability descriptors registered.
    pub fn list(&self) -> Vec<CapabilityDescriptor> {
        self.descriptors.clone()
    }
}

/// Structured diagnostics of operational health, recent failures, and shutdown telemetry.
#[derive(Debug, Clone)]
pub struct RuntimeDiagnostics {
    /// List of recent operational failures logged by the runtime (maximum 50 entries, FIFO).
    pub recent_failures: Vec<RuntimeFailure>,
    /// Summary of the last graceful shutdown operation, if completed.
    pub last_shutdown: Option<ShutdownSummary>,
}

/// A summary of a completed graceful shutdown.
#[derive(Debug, Clone)]
pub struct ShutdownSummary {
    /// The total duration it took to halt background threads and release storage.
    pub duration: Duration,
}

/// Represents an operational failure inside the runtime.
#[derive(Debug, Clone)]
pub struct RuntimeFailure {
    /// The name of the failed operation (e.g. "ingest").
    pub operation: String,
    /// Detailed error message.
    pub error: String,
    /// Absolute wall-clock time when the failure was logged.
    pub timestamp: SystemTime,
}

/// The outcome of a successful observation ingestion.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IngestionResult {
    /// Monotonic epoch representing the new state of the graph.
    pub epoch: EpochId,
    /// Entities created or mutated during this transaction.
    pub affected_entities: Vec<NodeId>,
    /// Detailed stage execution timings.
    pub stage_timings: StageTimings,
}

/// Quantitative operational metrics and completed execution timings.
#[derive(Debug, Clone)]
pub struct RuntimeMetrics {
    /// The total number of observations submitted for ingestion.
    pub observations_ingested: u64,
    /// The total number of successful knowledge canonicalization runs.
    pub canonicalization_successes: u64,
    /// The total number of failed canonicalization runs.
    pub canonicalization_failures: u64,
    /// The total number of relationship reflection processes executed.
    pub reflections_executed: u64,
    /// The total number of projection generation runs executed.
    pub projections_executed: u64,
    /// The total number of retrieval/search queries executed.
    pub retrieval_queries: u64,
    /// The duration of the most recently completed successful ingestion.
    pub last_ingest_duration: Option<Duration>,
    /// The duration of the most recently completed successful projection generation.
    pub last_projection_duration: Option<Duration>,

    // Sprint 8 — per-stage averages
    /// Cumulative average duration of the canonicalization stage.
    pub avg_canonicalization_duration: Option<Duration>,
    /// Cumulative average duration of the reflection stage.
    pub avg_reflection_duration: Option<Duration>,
    /// Cumulative average duration of the event dispatch stage.
    pub avg_dispatch_duration: Option<Duration>,
}

/// Representation of the stateful runtime health state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHealth {
    /// Runtime components are initializing.
    Initializing = 0,
    /// Runtime is active and ready to process requests.
    Healthy = 1,
    /// Shutdown has been triggered and event dispatchers are closing.
    ShuttingDown = 2,
    /// Shutdown has finished, services are stopped.
    Stopped = 3,
}

impl RuntimeHealth {
    fn from_u8(val: u8) -> Self {
        match val {
            0 => RuntimeHealth::Initializing,
            1 => RuntimeHealth::Healthy,
            2 => RuntimeHealth::ShuttingDown,
            _ => RuntimeHealth::Stopped,
        }
    }
}

/// Current status summary of the active runtime.
#[derive(Debug, Clone)]
pub struct RuntimeStatus {
    /// Monotonically increasing uptime since the runtime was instantiated.
    pub uptime: Duration,
    /// The backing storage engine implementation descriptor (e.g. "sqlite").
    pub storage_backend: String,
    /// Count of currently active sync and async event subscribers.
    pub active_event_subscribers: usize,
    /// Current type-safe health state of the engine.
    pub health: RuntimeHealth,
}

#[derive(Debug)]
pub(crate) struct InternalMetrics {
    pub(crate) observations_ingested: AtomicU64,
    pub(crate) canonicalization_successes: AtomicU64,
    pub(crate) canonicalization_failures: AtomicU64,
    pub(crate) reflections_executed: AtomicU64,
    pub(crate) projections_executed: AtomicU64,
    pub(crate) retrieval_queries: AtomicU64,
    pub(crate) last_ingest_duration_ns: AtomicU64,
    pub(crate) last_projection_duration_ns: AtomicU64,
    // Sprint 8 stage durations
    pub(crate) canonicalization_duration_ns: AtomicU64,
    pub(crate) reflection_duration_ns: AtomicU64,
    pub(crate) dispatch_duration_ns: AtomicU64,
}

pub(crate) struct InternalDiagnostics {
    pub(crate) recent_failures: VecDeque<RuntimeFailure>,
    pub(crate) last_shutdown: Option<ShutdownSummary>,
}

/// Unified composition root and lifecycle owner of the Brain Relational Engine.
pub struct BrainRuntime {
    storage: Arc<dyn Storage>,
    dispatcher: Arc<InMemoryEventDispatcher>,
    canonicalizer: Arc<dyn Canonicalizer<Error = BrainError>>,
    projection_manager: SqliteProjectionManager,
    correlation_index: Arc<Mutex<CorrelationIndex>>,
    subscriber: Option<ObservabilitySubscriber>,

    created_at: Instant,
    health: Arc<AtomicU8>,
    metrics: Arc<InternalMetrics>,
    diagnostics: Arc<Mutex<InternalDiagnostics>>,
    capabilities: CapabilityRegistry,
    // Concrete type used intentionally. A `Arc<dyn SearchRepository>` trait abstraction
    // is the correct direction if a second implementation appears (hybrid BM25+vector,
    // remote search, test double). Until then, the concrete type avoids a premature
    // abstraction with no second case to justify it. See: governing principle, Sprint 6.
    //
    // When introducing the trait: update this comment first, then revise Sprint 8 plan
    // "Future Consideration" section (or reference the ADR if one exists). This comment
    // is the authoritative description of the current design; the sprint doc is the
    // historical record of why this decision was made.
    search_repository: Arc<SqliteSearchRepository>,
}

impl BrainRuntime {
    /// Constructs the complete runtime from a database path.
    ///
    /// Components are initialized in dependency order:
    /// 1. Storage (connection pool — must exist before any service touches the database)
    /// 2. Event dispatcher (must exist before subscribers or services that emit events)
    /// 3. Observability subscriber (wired before any events are dispatched)
    /// 4. Reflection engine (depends on storage + dispatcher)
    /// 5. Canonicalizer with reflection (depends on storage + dispatcher + engine)
    /// 6. Projection manager (depends on storage + dispatcher)
    ///
    /// Uses sane defaults: pool_size=4, WAL enabled.
    ///
    /// **Exception safety**: Rust's ownership model ensures partially initialized resources
    /// drop in reverse initialization order if construction fails.
    pub fn new(db_path: &str) -> Result<Self, BrainError> {
        let created_at = Instant::now();
        let health = Arc::new(AtomicU8::new(RuntimeHealth::Initializing as u8));
        let metrics = Arc::new(InternalMetrics {
            observations_ingested: AtomicU64::new(0),
            canonicalization_successes: AtomicU64::new(0),
            canonicalization_failures: AtomicU64::new(0),
            reflections_executed: AtomicU64::new(0),
            projections_executed: AtomicU64::new(0),
            retrieval_queries: AtomicU64::new(0),
            last_ingest_duration_ns: AtomicU64::new(0),
            last_projection_duration_ns: AtomicU64::new(0),
            canonicalization_duration_ns: AtomicU64::new(0),
            reflection_duration_ns: AtomicU64::new(0),
            dispatch_duration_ns: AtomicU64::new(0),
        });
        let diagnostics = Arc::new(Mutex::new(InternalDiagnostics {
            recent_failures: VecDeque::new(),
            last_shutdown: None,
        }));

        let mut capabilities = CapabilityRegistry::new();

        capabilities.register(CapabilityDescriptor {
            name: "storage",
            version: 1,
            description: "Durable SQLite relational storage and transaction engine",
            state: CapabilityState::Active,
            is_enabled: true,
            is_experimental: false,
        })?;

        capabilities.register(CapabilityDescriptor {
            name: "evolution",
            version: 1,
            description:
                "Observation validation, canonicalization, and relationship reflection pipeline",
            state: CapabilityState::Active,
            is_enabled: true,
            is_experimental: false,
        })?;

        capabilities.register(CapabilityDescriptor {
            name: "projection",
            version: 1,
            description: "Materialized view management and event-driven read-model projection query boundary",
            state: CapabilityState::Active,
            is_enabled: true,
            is_experimental: false,
        })?;

        capabilities.register(CapabilityDescriptor {
            name: "subscription",
            version: 1,
            description: "Event bus for async stream and sync timeline observers",
            state: CapabilityState::Active,
            is_enabled: true,
            is_experimental: false,
        })?;

        // 1. Storage — hold concrete type first so pool can be shared with search repository
        let sqlite_storage = Arc::new(SqliteStorage::new(db_path, 4, true)?);
        let search_repository =
            Arc::new(SqliteSearchRepository::new(sqlite_storage.pool().clone()));
        let storage: Arc<dyn Storage> = Arc::clone(&sqlite_storage) as Arc<dyn Storage>;

        // 2. Event dispatcher — concrete type held privately; exposed only via trait object
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(64));
        let dispatcher_trait: Arc<dyn RuntimeEventDispatcher> =
            Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;

        // 3. Observability subscriber — must be subscribed before any events are dispatched
        let correlation_index = Arc::new(Mutex::new(CorrelationIndex::new()));
        let sync_rx = dispatcher.subscribe_sync();
        let subscriber = ObservabilitySubscriber::new(sync_rx, Arc::clone(&correlation_index));

        // 4. Reflection engine
        let reflection_engine = Arc::new(
            SqliteReflectionEngine::new(Arc::clone(&storage), Arc::clone(&dispatcher_trait))
                .with_metrics(Arc::clone(&metrics)),
        );

        // 5. Canonicalizer with reflection
        let canonicalizer: Arc<dyn Canonicalizer<Error = BrainError>> = Arc::new(
            SqliteCanonicalizer::new(Arc::clone(&storage), Arc::clone(&dispatcher_trait))
                .with_reflection(reflection_engine),
        );

        // 6. Projection manager
        let epoch = Arc::new(Mutex::new(EpochId::initial()));
        let projection_manager = SqliteProjectionManager::new(
            Arc::clone(&storage),
            epoch,
            Arc::clone(&dispatcher_trait),
        );

        health.store(RuntimeHealth::Healthy as u8, Ordering::Release);

        Ok(Self {
            storage,
            dispatcher,
            canonicalizer,
            projection_manager,
            correlation_index,
            subscriber: Some(subscriber),
            created_at,
            health,
            metrics,
            diagnostics,
            capabilities,
            search_repository,
        })
    }

    /// Primary Ingestion boundary. Coordinates validation, canonicalization, and reflection.
    pub fn ingest(&self, obs: Observation) -> Result<IngestionResult, BrainError> {
        let start = Instant::now();
        self.metrics
            .observations_ingested
            .fetch_add(1, Ordering::Relaxed);

        match self.canonicalizer.canonicalize(obs) {
            Ok(result) => {
                let duration = start.elapsed();
                self.metrics
                    .canonicalization_successes
                    .fetch_add(1, Ordering::Relaxed);
                self.metrics
                    .last_ingest_duration_ns
                    .store(duration.as_nanos() as u64, Ordering::Release);

                // Accumulate durations
                self.metrics.canonicalization_duration_ns.fetch_add(
                    result.stage_timings.canonicalization.as_nanos() as u64,
                    Ordering::Relaxed,
                );
                self.metrics.reflection_duration_ns.fetch_add(
                    result.stage_timings.reflection.as_nanos() as u64,
                    Ordering::Relaxed,
                );
                self.metrics.dispatch_duration_ns.fetch_add(
                    result.stage_timings.dispatch.as_nanos() as u64,
                    Ordering::Relaxed,
                );

                Ok(IngestionResult {
                    epoch: result.epoch,
                    affected_entities: result.affected_entities,
                    stage_timings: result.stage_timings,
                })
            }
            Err(err) => {
                self.metrics
                    .canonicalization_failures
                    .fetch_add(1, Ordering::Relaxed);

                // Add to diagnostics ring buffer
                let failure = RuntimeFailure {
                    operation: "ingest".to_string(),
                    error: err.to_string(),
                    timestamp: SystemTime::now(),
                };
                let mut diag = self.diagnostics.lock().unwrap();
                diag.recent_failures.push_back(failure);
                if diag.recent_failures.len() > 50 {
                    diag.recent_failures.pop_front();
                }

                Err(err)
            }
        }
    }

    /// Unified projection query boundary.
    pub fn query_projection<P, Q: ProjectionQuery, PR: Projector<P, Q>>(
        &self,
        projector: &PR,
        query: &Q,
        correlation_id: CorrelationId,
    ) -> P {
        let start = Instant::now();
        let res = self
            .projection_manager
            .project(projector, query, correlation_id);
        let duration = start.elapsed();
        self.metrics
            .projections_executed
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .last_projection_duration_ns
            .store(duration.as_nanos() as u64, Ordering::Release);
        res
    }

    /// Allows hosts or adapters to subscribe to runtime event stream.
    pub fn subscribe(&self) -> tokio::sync::mpsc::Receiver<Arc<dyn RuntimeEvent>> {
        self.dispatcher.subscribe()
    }

    /// Exposes read-only facade query for correlation index spans.
    pub fn spans_for(&self, corr_id: CorrelationId) -> Option<Vec<OperationSpan>> {
        let index = self.correlation_index.lock().unwrap();
        index.spans_for(corr_id).map(|spans| spans.to_vec())
    }

    /// Exposes read-only facade query for checking correlation completeness.
    pub fn is_complete(&self, corr_id: CorrelationId) -> bool {
        let index = self.correlation_index.lock().unwrap();
        index.is_complete(corr_id)
    }

    /// Exposes read-only storage reference.
    ///
    /// **Testing support only**: Not intended as a host extension API. Use only for test query assertions.
    pub fn storage_ref(&self) -> Arc<dyn Storage> {
        Arc::clone(&self.storage)
    }

    /// Query the current type-safe status snapshot. Cheap and non-blocking.
    pub fn status(&self) -> RuntimeStatus {
        let active_event_subscribers = self.dispatcher.active_subscribers_count();
        let health_val = self.health.load(Ordering::Acquire);
        let health = RuntimeHealth::from_u8(health_val);
        let uptime = self.created_at.elapsed();

        RuntimeStatus {
            uptime,
            storage_backend: "sqlite".to_string(),
            active_event_subscribers,
            health,
        }
    }

    /// Query the current quantitative metrics snapshot. Cheap and non-blocking.
    pub fn metrics(&self) -> RuntimeMetrics {
        let observations_ingested = self.metrics.observations_ingested.load(Ordering::Acquire);
        let canonicalization_successes = self
            .metrics
            .canonicalization_successes
            .load(Ordering::Acquire);
        let canonicalization_failures = self
            .metrics
            .canonicalization_failures
            .load(Ordering::Acquire);
        let reflections_executed = self.metrics.reflections_executed.load(Ordering::Acquire);
        let projections_executed = self.metrics.projections_executed.load(Ordering::Acquire);

        let ingest_ns = self.metrics.last_ingest_duration_ns.load(Ordering::Acquire);
        let last_ingest_duration = if ingest_ns > 0 {
            Some(Duration::from_nanos(ingest_ns))
        } else {
            None
        };

        let projection_ns = self
            .metrics
            .last_projection_duration_ns
            .load(Ordering::Acquire);
        let last_projection_duration = if projection_ns > 0 {
            Some(Duration::from_nanos(projection_ns))
        } else {
            None
        };

        let retrieval_queries = self.metrics.retrieval_queries.load(Ordering::Acquire);

        let avg_canonicalization_duration = self
            .metrics
            .canonicalization_duration_ns
            .load(Ordering::Acquire)
            .checked_div(canonicalization_successes)
            .map(Duration::from_nanos);

        let avg_reflection_duration = self
            .metrics
            .reflection_duration_ns
            .load(Ordering::Acquire)
            .checked_div(canonicalization_successes)
            .map(Duration::from_nanos);

        let avg_dispatch_duration = self
            .metrics
            .dispatch_duration_ns
            .load(Ordering::Acquire)
            .checked_div(canonicalization_successes)
            .map(Duration::from_nanos);

        RuntimeMetrics {
            observations_ingested,
            canonicalization_successes,
            canonicalization_failures,
            reflections_executed,
            projections_executed,
            retrieval_queries,
            last_ingest_duration,
            last_projection_duration,
            avg_canonicalization_duration,
            avg_reflection_duration,
            avg_dispatch_duration,
        }
    }

    /// Query the current diagnostics failure log snapshot. Non-blocking with short lock acquisition.
    pub fn diagnostics(&self) -> RuntimeDiagnostics {
        let diag = self.diagnostics.lock().unwrap();
        RuntimeDiagnostics {
            recent_failures: diag.recent_failures.iter().cloned().collect(),
            last_shutdown: diag.last_shutdown.clone(),
        }
    }

    /// Discover all capabilities supported by the runtime, stably sorted alphabetically by name.
    /// Cheap, non-blocking snapshot query.
    pub fn discover_capabilities(&self) -> Vec<CapabilityDescriptor> {
        let mut list = self.capabilities.list();
        list.sort_by_key(|c| c.name);
        list
    }

    /// Runs a search query over the relational knowledge database, returning pure domain-level documents.
    pub fn search(&self, query: SearchQuery) -> Result<Vec<SearchDocument>, BrainError> {
        self.metrics
            .retrieval_queries
            .fetch_add(1, Ordering::Relaxed);
        let storage_query = brain_storage::SearchQuery {
            text: query.text,
            kinds: query.kinds,
            limit: query.pagination.as_ref().and_then(|p| p.limit),
            offset: query.pagination.as_ref().and_then(|p| p.offset),
        };
        self.search_repository.search(&storage_query)
    }

    /// Lifecycle boundary: stops workers, flushes event queues, and closes storage connections.
    /// Consumes `self` to statically guarantee no further actions can be invoked after shutdown.
    ///
    /// **Teardown Invariants**:
    /// 1. Close event dispatcher channels first (rejects new work, drops all SyncSenders/Senders).
    ///    This sends the explicit termination signal to the observability subscriber thread by closing the channel.
    /// 2. Drop/take the subscriber. The dropped subscriber triggers join on the observability background thread.
    /// 3. Release/drop the SQLite storage connection pool.
    pub fn shutdown(mut self) -> Result<ShutdownSummary, BrainError> {
        let start = Instant::now();
        self.health
            .store(RuntimeHealth::ShuttingDown as u8, Ordering::Release);

        // 1. Close all event channels — thread unblocks from recv(), sees Disconnected, exits
        self.dispatcher.shutdown();

        // 2. Join the observability thread
        if let Some(sub) = self.subscriber.take() {
            drop(sub);
        }

        // 3. Release SQLite connection pool
        drop(self.storage);

        let duration = start.elapsed();
        self.health
            .store(RuntimeHealth::Stopped as u8, Ordering::Release);

        let summary = ShutdownSummary { duration };
        if let Ok(mut diag) = self.diagnostics.lock() {
            diag.last_shutdown = Some(summary.clone());
        }

        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duplicate_capability_registration_fails() {
        let mut registry = CapabilityRegistry::new();

        let desc1 = CapabilityDescriptor {
            name: "test_cap",
            version: 1,
            description: "Test description",
            state: CapabilityState::Active,
            is_enabled: true,
            is_experimental: false,
        };
        registry.register(desc1.clone()).unwrap();

        // Attempting to register the same name again should fail
        let desc2 = CapabilityDescriptor {
            name: "test_cap",
            version: 2,
            description: "Another description",
            state: CapabilityState::Inactive,
            is_enabled: false,
            is_experimental: true,
        };
        let err = registry.register(desc2).unwrap_err();
        assert!(matches!(err, BrainError::InvalidTransition { .. }));
        assert!(err.to_string().contains("already registered"));
    }
}
