//! First-class reflection runtime event vocabulary, versioned envelopes, bus subscriber channel, and delivery contracts.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Current schema version for `ReflectionEventEnvelope`.
pub const CURRENT_EVENT_SCHEMA_VERSION: u32 = 1;

/// Versioned metadata envelope wrapping a `ReflectionRuntimeEvent` with tracing correlation context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionEventEnvelope {
    /// Schema format version for event compatibility validation.
    pub schema_version: u32,
    /// Globally unique event identifier.
    pub event_id: Uuid,
    /// Execution plan identifier.
    pub plan_id: String,
    /// Optional stable task identifier string.
    pub task_id: Option<String>,
    /// Distributed tracing correlation identifier.
    pub correlation_id: Uuid,
    /// Event timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Immutable configuration version under which event occurred (Phase G.5).
    pub config_version: u64,
    /// Core event payload variant.
    pub event: ReflectionRuntimeEvent,
}

impl ReflectionEventEnvelope {
    /// Wraps a runtime event in a versioned metadata envelope with a new unique event ID.
    pub fn new(
        plan_id: impl Into<String>,
        task_id: Option<String>,
        correlation_id: Uuid,
        timestamp_ms: u64,
        event: ReflectionRuntimeEvent,
    ) -> Self {
        Self {
            schema_version: CURRENT_EVENT_SCHEMA_VERSION,
            event_id: Uuid::new_v4(),
            plan_id: plan_id.into(),
            task_id,
            correlation_id,
            timestamp_ms,
            config_version: 1,
            event,
        }
    }

    /// Explicitly binds a specific configuration version index to the envelope.
    pub fn with_config_version(mut self, config_version: u64) -> Self {
        self.config_version = config_version;
        self
    }
}

/// Structured runtime event vocabulary for reflection execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReflectionRuntimeEvent {
    /// Task execution started.
    TaskStarted {
        /// Execution plan identifier.
        plan_id: String,
        /// Current stage index.
        stage_index: usize,
        /// Stable task identifier string.
        task_id: String,
        /// Event timestamp in milliseconds.
        timestamp_ms: u64,
    },
    /// Task execution completed successfully.
    TaskCompleted {
        /// Execution plan identifier.
        plan_id: String,
        /// Current stage index.
        stage_index: usize,
        /// Stable task identifier string.
        task_id: String,
        /// Task execution latency in milliseconds.
        duration_ms: u64,
        /// Count of structural or state changes applied.
        changes_applied: usize,
        /// Event timestamp in milliseconds.
        timestamp_ms: u64,
    },
    /// Task execution attempt failed and is being retried.
    TaskRetried {
        /// Execution plan identifier.
        plan_id: String,
        /// Current stage index.
        stage_index: usize,
        /// Stable task identifier string.
        task_id: String,
        /// 1-based attempt index.
        attempt: u32,
        /// Failure error details message.
        error_message: String,
        /// Event timestamp in milliseconds.
        timestamp_ms: u64,
    },
    /// Stage checkpoint saved.
    CheckpointCreated {
        /// Execution plan identifier.
        plan_id: String,
        /// Last completed stage index.
        stage_index: usize,
        /// Cumulative modified entity count.
        modified_entity_count: usize,
        /// Event timestamp in milliseconds.
        timestamp_ms: u64,
    },
    /// Resumable recovery started.
    RecoveryStarted {
        /// Execution plan identifier.
        plan_id: String,
        /// Stage index to resume execution from.
        resuming_stage_index: usize,
        /// Event timestamp in milliseconds.
        timestamp_ms: u64,
    },
    /// Resumable recovery completed.
    RecoveryCompleted {
        /// Execution plan identifier.
        plan_id: String,
        /// Stage index resumed at.
        resumed_stage_index: usize,
        /// Event timestamp in milliseconds.
        timestamp_ms: u64,
    },
}

/// Subscriber callback handler for reflection event envelopes.
pub type ReflectionEventSubscriber = Box<dyn Fn(&Arc<ReflectionEventEnvelope>) + Send + Sync>;

/// Thread-safe event bus channel for reflection runtime progress observability.
#[derive(Default)]
pub struct ReflectionEventBus {
    subscribers: Mutex<Vec<ReflectionEventSubscriber>>,
}

impl ReflectionEventBus {
    /// Creates a new empty `ReflectionEventBus`.
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    /// Subscribes a listener callback to published reflection event envelopes.
    pub fn subscribe<F>(&self, subscriber: F)
    where
        F: Fn(&Arc<ReflectionEventEnvelope>) + Send + Sync + 'static,
    {
        let mut subs = self.subscribers.lock().expect("Event bus lock poisoned");
        subs.push(Box::new(subscriber));
    }

    /// Publishes a `ReflectionEventEnvelope` to all registered subscribers with isolated invocation.
    pub fn publish(&self, envelope: ReflectionEventEnvelope) {
        let payload = Arc::new(envelope);
        let subs = self.subscribers.lock().expect("Event bus lock poisoned");
        for sub in subs.iter() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sub(&payload);
            }));
        }
    }
}
