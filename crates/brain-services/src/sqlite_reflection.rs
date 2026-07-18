//! SQLite-backed reflection engine implementation.
//!
//! Enforces that confidence calculations and invariants stay inside the domain model
//! (calling `edge.strengthen_with_evidence()`), while coordinating database transaction
//! and dispatching the resulting `RuntimeRelationshipEvent` and projection invalidations.

use brain_core::{
    events::{
        EventSource, ProjectionInstanceInvalidatedEvent, RuntimeEventDispatcher,
        RuntimeRelationshipEvent,
    },
    reflection::{ReflectionCompletedEvent, ReflectionEngine, ReflectionTarget},
    repositories::Storage,
};
use brain_domain::relations::RelationRegistry;
use std::sync::Arc;
use std::time::SystemTime;

/// SQLite production implementation of the `ReflectionEngine` contract.
///
/// Coordinates the workflow of retrieving adjacent edges, lookup of ontology configuration,
/// calling domain invariants on Edge to calculate updated weights, persisting changes, and
/// dispatching event signals.
pub struct SqliteReflectionEngine {
    storage: Arc<dyn Storage>,
    event_dispatcher: Arc<dyn RuntimeEventDispatcher>,
    metrics: Option<Arc<crate::brain_runtime::InternalMetrics>>,
}

impl SqliteReflectionEngine {
    /// Creates a new `SqliteReflectionEngine`.
    pub fn new(
        storage: Arc<dyn Storage>,
        event_dispatcher: Arc<dyn RuntimeEventDispatcher>,
    ) -> Self {
        Self {
            storage,
            event_dispatcher,
            metrics: None,
        }
    }

    /// Attaches metrics collection to the reflection engine.
    pub(crate) fn with_metrics(
        mut self,
        metrics: Arc<crate::brain_runtime::InternalMetrics>,
    ) -> Self {
        self.metrics = Some(metrics);
        self
    }
}

impl ReflectionEngine for SqliteReflectionEngine {
    type Error = brain_core::errors::BrainError;

    fn reflect(&self, target: ReflectionTarget) -> Result<ReflectionCompletedEvent, Self::Error> {
        let registry = RelationRegistry::default_embedded();
        let mut strengthened_events = Vec::new();

        // Perform reflection within a transaction to maintain atomicity.
        self.storage.run_transaction(&mut |tx| {
            let repos = tx.repositories();

            for node_id in &target.affected_entities {
                // Get all adjacent incoming and outgoing edges for the node
                let connections = repos.edges().get_connections(node_id)?;
                for mut edge in connections {
                    // Consult the ontology registry for relationship definition/strategy
                    if let Some(def) = registry.get(edge.relation.id()) {
                        // Invoke domain entity to apply confidence strategy (no direct weight manipulation)
                        match edge.strengthen_with_evidence(1.0, def.confidence_strategy) {
                            Ok(domain_ev) => {
                                repos.edges().save(&edge)?;
                                strengthened_events.push(domain_ev);
                            }
                            Err(e) => {
                                return Err(brain_core::errors::BrainError::Validation {
                                    message: format!(
                                        "Domain validation failed during edge strengthening: {:?}",
                                        e
                                    ),
                                });
                            }
                        }
                    }
                }
            }
            Ok(())
        })?;

        // Dispatch adaptors for all successfully committed strengthened relationship events.
        for domain_event in strengthened_events {
            self.event_dispatcher
                .dispatch(Arc::new(RuntimeRelationshipEvent {
                    domain_event,
                    epoch: target.epoch,
                    correlation_id: target.correlation_id,
                    timestamp: SystemTime::now(),
                }));
        }

        // Signal invalidations for all affected projection instances.
        for entity_id in &target.affected_entities {
            self.event_dispatcher
                .dispatch(Arc::new(ProjectionInstanceInvalidatedEvent {
                    projection_type: format!("entity:{}", entity_id),
                    epoch: target.epoch,
                    source: EventSource::Reflection,
                    correlation_id: target.correlation_id,
                }));
        }

        // Construct and dispatch durable completion event.
        let event = ReflectionCompletedEvent {
            epoch: target.epoch,
            entities_reflected: target.affected_entities.clone(),
            correlation_id: target.correlation_id,
            timestamp: SystemTime::now(),
        };

        self.event_dispatcher.dispatch(Arc::new(event.clone()));

        if let Some(ref m) = self.metrics {
            m.reflections_executed
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        Ok(event)
    }
}
