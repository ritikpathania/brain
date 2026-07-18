use crate::event_dispatcher::InMemoryEventDispatcher;
use brain_core::{
    events::{
        CorrelationId, EventSource, ProjectionInstanceInvalidatedEvent, RuntimeEventDispatcher,
    },
    projection::{ProjectionContext, ProjectionQuery, Projector},
};
use brain_domain::{EpochId, KnowledgeGraph};
use std::sync::{Arc, Mutex};

/// Coordinator of projectors that provides direct on-demand projection building and invalidation dispatching.
pub struct ProjectionManager {
    graph: Arc<Mutex<KnowledgeGraph>>,
    epoch: Arc<Mutex<EpochId>>,
    event_dispatcher: Arc<InMemoryEventDispatcher>,
}

impl ProjectionManager {
    /// Creates a new `ProjectionManager`.
    pub fn new(
        graph: Arc<Mutex<KnowledgeGraph>>,
        epoch: Arc<Mutex<EpochId>>,
        event_dispatcher: Arc<InMemoryEventDispatcher>,
    ) -> Self {
        Self {
            graph,
            epoch,
            event_dispatcher,
        }
    }

    /// Rebuilds and returns the projection on-demand from the current canonical state.
    pub fn project<P, Q: ProjectionQuery, PR: Projector<P, Q>>(
        &self,
        projector: &PR,
        query: &Q,
        correlation_id: CorrelationId,
    ) -> P {
        let graph_lock = self.graph.lock().unwrap();
        let epoch_lock = self.epoch.lock().unwrap();

        let context = ProjectionContext {
            graph: &*graph_lock,
            epoch: *epoch_lock,
            query,
            correlation_id,
        };

        projector.project(&context)
    }

    /// Synchronously invalidates one or more projection instances and dispatches the invalidation event.
    pub fn invalidate(&self, projection_type: String, correlation_id: CorrelationId) {
        let epoch_lock = self.epoch.lock().unwrap();
        let current_epoch = *epoch_lock;

        self.event_dispatcher
            .dispatch(Arc::new(ProjectionInstanceInvalidatedEvent {
                projection_type,
                epoch: current_epoch,
                source: EventSource::Projection,
                correlation_id,
            }));
    }
}
