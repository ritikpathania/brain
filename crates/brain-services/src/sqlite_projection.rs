use crate::memory_list_projection::{MemoryListProjection, MemoryListQuery};
use brain_core::{
    events::{CorrelationId, RuntimeEventDispatcher},
    projection::{ProjectionContext, ProjectionQuery, Projector},
    repositories::Storage,
};
use brain_domain::{EpochId, KnowledgeGraph, NodeType};
use std::sync::{Arc, Mutex};

/// Concrete projector executing over persisted database records.
pub struct SqliteProjector {
    storage: Arc<dyn Storage>,
}

impl SqliteProjector {
    /// Creates a new `SqliteProjector`.
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }
}

impl Projector<MemoryListProjection, MemoryListQuery> for SqliteProjector {
    fn project(&self, context: &ProjectionContext<MemoryListQuery>) -> MemoryListProjection {
        // Query the abstract repository interface
        // CONSTRAINT: Absolutely no SQL is written here.
        let mut nodes = Vec::new();
        let _ = self.storage.run_transaction(&mut |tx| {
            if let Ok(n) = tx.repositories().nodes().list_all() {
                nodes = n;
            }
            Ok(())
        });

        // Filter concept nodes and sort them deterministically
        let mut items: Vec<_> = nodes
            .into_iter()
            .filter(|n| n.node_type == NodeType::Concept)
            .collect();

        items.sort_by(|a, b| a.label.cmp(&b.label));

        if items.len() > context.query.limit {
            items.truncate(context.query.limit);
        }

        MemoryListProjection { items }
    }
}

/// Coordinate projection builders and serve on-demand SQLite-backed projection generation.
///
/// The event dispatcher field is typed as `Arc<dyn RuntimeEventDispatcher>` — the concrete
/// implementation is hidden behind the contract.
pub struct SqliteProjectionManager {
    storage: Arc<dyn Storage>,
    _epoch: Arc<Mutex<EpochId>>,
    _event_dispatcher: Arc<dyn RuntimeEventDispatcher>,
}

impl SqliteProjectionManager {
    /// Creates a new `SqliteProjectionManager`.
    pub fn new(
        storage: Arc<dyn Storage>,
        epoch: Arc<Mutex<EpochId>>,
        event_dispatcher: Arc<dyn RuntimeEventDispatcher>,
    ) -> Self {
        Self {
            storage,
            _epoch: epoch,
            _event_dispatcher: event_dispatcher,
        }
    }

    /// Serves on-demand projection generation by fetching the active epoch and reconstructing the graph context.
    pub fn project<P, Q: ProjectionQuery, PR: Projector<P, Q>>(
        &self,
        projector: &PR,
        query: &Q,
        correlation_id: CorrelationId,
    ) -> P {
        // Retrieve current epoch from configuration repository
        let mut current_epoch = EpochId::initial();
        let _ = self.storage.run_transaction(&mut |tx| {
            let epoch_str = tx.repositories().configs().get_key("current_epoch")?;
            if let Some(s) = epoch_str {
                if let Ok(val) = s.parse::<u64>() {
                    current_epoch = EpochId(val);
                }
            }
            Ok(())
        });

        // Reconstruct the graph context snapshot from SQLite database transaction
        let mut graph = KnowledgeGraph::new();
        let _ = self.storage.run_transaction(&mut |tx| {
            if let Ok(nodes) = tx.repositories().nodes().list_all() {
                for n in nodes {
                    graph.nodes.insert(n.id, n);
                }
            }
            Ok(())
        });

        let context = ProjectionContext {
            graph: &graph,
            epoch: current_epoch,
            query,
            correlation_id,
        };

        projector.project(&context)
    }
}
