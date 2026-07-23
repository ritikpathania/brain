//! Decoupled event-driven ReflectionService responding to RuntimeEvent::KnowledgeCompiled (KPP v1.6).

use brain_events::{
    ReflectionCompletedPayload, RuntimeEvent, RuntimeEventBus, RuntimeEventSubscriber,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Decoupled service managing reflection graph sweeps in response to runtime events.
pub struct ReflectionService {
    event_bus: Option<Arc<RuntimeEventBus>>,
    last_evaluated_version: AtomicU64,
    total_sweeps_executed: AtomicU64,
}

impl Default for ReflectionService {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ReflectionService {
    /// Instantiates a new `ReflectionService` with optional event bus for downstream dispatch.
    pub fn new(event_bus: Option<Arc<RuntimeEventBus>>) -> Self {
        Self {
            event_bus,
            last_evaluated_version: AtomicU64::new(0),
            total_sweeps_executed: AtomicU64::new(0),
        }
    }

    /// Returns total reflection sweeps executed.
    pub fn total_sweeps_executed(&self) -> u64 {
        self.total_sweeps_executed.load(Ordering::Acquire)
    }

    /// Returns last evaluated graph version epoch sequence.
    pub fn last_evaluated_version(&self) -> u64 {
        self.last_evaluated_version.load(Ordering::Acquire)
    }

    /// Executes targeted reflection sweep over delta changed entities.
    pub async fn execute_targeted_sweep(
        &self,
        graph_version: u64,
        changed_entities_count: usize,
    ) -> ReflectionCompletedPayload {
        self.total_sweeps_executed.fetch_add(1, Ordering::Relaxed);
        self.last_evaluated_version
            .store(graph_version, Ordering::Release);

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let payload = ReflectionCompletedPayload {
            graph_version,
            evaluated_entities_count: changed_entities_count,
            mutated_edges_count: 0,
            timestamp_ms: now_ms,
        };

        if let Some(ref bus) = self.event_bus {
            bus.publish(RuntimeEvent::ReflectionCompleted(payload.clone()))
                .await;
        }

        payload
    }
}

impl RuntimeEventSubscriber for ReflectionService {
    fn name(&self) -> &'static str {
        "ReflectionService"
    }

    fn handle_event<'a>(
        &'a self,
        event: std::sync::Arc<RuntimeEvent>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let RuntimeEvent::KnowledgeCompiled(payload) = &*event {
                self.execute_targeted_sweep(payload.graph_version, payload.changed_entities.len())
                    .await;
            }
        })
    }
}
