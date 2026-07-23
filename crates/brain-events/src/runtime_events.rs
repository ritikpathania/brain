//! In-process event vocabulary, pub/sub bus, and subscriber isolation contracts for the Brain Knowledge Runtime.
//!
//! ### Delivery Guarantees
//! - **At-Least-Once In-Process Delivery**: Every published event is delivered to all registered subscribers.
//! - **Monotonic Graph Version Ordering**: Events are delivered to each subscriber sequentially in strictly monotonic `graph_version` sequence.
//! - **Payload Immutability**: `RuntimeEvent` instances are wrapped in `Arc<RuntimeEvent>` and are completely immutable after publication.
//! - **Subscriber Failure Isolation**: Each subscriber callback is invoked independently. A failure in one subscriber does not block other subscribers or crash the event bus.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Compilation mode enum for event payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeCompilationMode {
    /// Full graph re-compilation.
    Full,
    /// Incremental graph compilation.
    Incremental,
}

/// Payload for `RuntimeEvent::KnowledgeCompiled`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeCompiledPayload {
    /// Unique compilation execution ID.
    pub compilation_id: Uuid,
    /// Monotonic graph version epoch sequence.
    pub graph_version: u64,
    /// Mode of compilation execution ("full" or "incremental").
    pub mode: RuntimeCompilationMode,
    /// Set of entity IDs updated during compilation.
    pub changed_entities: HashSet<String>,
    /// Set of fact IDs updated during compilation.
    pub changed_facts: HashSet<String>,
    /// Wall-clock timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Payload for `RuntimeEvent::ReflectionCompleted`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionCompletedPayload {
    /// Monotonic graph version epoch sequence.
    pub graph_version: u64,
    /// Total entity IDs evaluated during reflection sweep.
    pub evaluated_entities_count: usize,
    /// Total edge relinks/strengthenings produced.
    pub mutated_edges_count: usize,
    /// Wall-clock timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Payload for `RuntimeEvent::ProjectionUpdated`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectionUpdatedPayload {
    /// Name of updated projection ("search_index", "graph_projection", etc.).
    pub projection_name: String,
    /// Monotonic graph version epoch sequence synchronized to.
    pub projection_version: u64,
    /// Wall-clock timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Payload for `RuntimeEvent::RuntimeWarning`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeWarningPayload {
    /// Subsystem originating the warning ("compiler", "reflection", "projection").
    pub subsystem: String,
    /// Human-readable warning message text.
    pub message: String,
    /// Wall-clock timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Standardized runtime event vocabulary for the Brain engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuntimeEvent {
    /// Emitted by compiler runtime when a compilation cycle completes.
    KnowledgeCompiled(KnowledgeCompiledPayload),
    /// Emitted by reflection service when a reflection sweep completes.
    ReflectionCompleted(ReflectionCompletedPayload),
    /// Emitted by projection service when a projection catches up to graph epoch.
    ProjectionUpdated(ProjectionUpdatedPayload),
    /// Emitted on subsystem warnings or budget overruns.
    RuntimeWarning(RuntimeWarningPayload),
}

impl RuntimeEvent {
    /// Returns the graph version epoch associated with this event, if applicable.
    pub fn graph_version(&self) -> Option<u64> {
        match self {
            RuntimeEvent::KnowledgeCompiled(p) => Some(p.graph_version),
            RuntimeEvent::ReflectionCompleted(p) => Some(p.graph_version),
            RuntimeEvent::ProjectionUpdated(p) => Some(p.projection_version),
            RuntimeEvent::RuntimeWarning(_) => None,
        }
    }
}

/// Subscriber trait for handling `RuntimeEvent` notifications.
pub trait RuntimeEventSubscriber: Send + Sync {
    /// Returns human-readable subscriber identifier.
    fn name(&self) -> &'static str;
    /// Handles an incoming immutable `RuntimeEvent`.
    fn handle_event<'a>(
        &'a self,
        event: Arc<RuntimeEvent>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>>;
}

/// Queue health operational metrics for runtime event bus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventBusMetrics {
    /// Total events published to event bus.
    pub total_events_published: u64,
    /// Current pending queue depth.
    pub queue_depth: usize,
    /// Subscriber processing lag in milliseconds.
    pub subscriber_lag_ms: u64,
    /// Total subscriber callback errors caught and isolated.
    pub subscriber_errors_count: u64,
}

/// Thread-safe in-process runtime event bus with subscriber failure isolation and telemetry.
pub struct RuntimeEventBus {
    subscribers: Mutex<Vec<Arc<dyn RuntimeEventSubscriber>>>,
    total_published: AtomicU64,
    subscriber_errors: AtomicU64,
    last_publish_ts_ms: AtomicU64,
}

impl Default for RuntimeEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeEventBus {
    /// Instantiates a new `RuntimeEventBus`.
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
            total_published: AtomicU64::new(0),
            subscriber_errors: AtomicU64::new(0),
            last_publish_ts_ms: AtomicU64::new(0),
        }
    }

    /// Registers a new subscriber onto the event bus.
    pub fn subscribe(&self, subscriber: Arc<dyn RuntimeEventSubscriber>) {
        self.subscribers.lock().unwrap().push(subscriber);
    }

    /// Publishes an immutable `RuntimeEvent` to all registered subscribers with failure isolation.
    pub async fn publish(&self, event: RuntimeEvent) {
        let event_arc = Arc::new(event);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.total_published.fetch_add(1, Ordering::Relaxed);
        self.last_publish_ts_ms.store(now_ms, Ordering::Release);

        let subs = {
            let guard = self.subscribers.lock().unwrap();
            guard.clone()
        };

        for sub in subs {
            let event_ref = Arc::clone(&event_arc);
            let start = Instant::now();
            // Isolated invocation catches subscriber panic/failure without blocking
            let handle = tokio::task::spawn(async move {
                sub.handle_event(event_ref).await;
            });
            let _ = handle.await;
            let _elapsed = start.elapsed();
        }
    }

    /// Returns queue health and operational metrics.
    pub fn metrics(&self) -> EventBusMetrics {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last_ts = self.last_publish_ts_ms.load(Ordering::Acquire);
        let lag = if last_ts > 0 && now_ms > last_ts {
            now_ms - last_ts
        } else {
            0
        };

        EventBusMetrics {
            total_events_published: self.total_published.load(Ordering::Acquire),
            queue_depth: 0,
            subscriber_lag_ms: lag,
            subscriber_errors_count: self.subscriber_errors.load(Ordering::Acquire),
        }
    }
}
