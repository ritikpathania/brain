use std::sync::{Arc, Mutex};
use brain_core::{
    events::{CorrelationId, RuntimeEventDispatcher},
    projection::{ProjectionQuery, Projector, ProjectionContext}
};
use brain_domain::{EpochId, KnowledgeGraph, NodeType};
use brain_storage::SqliteStorage;
use crate::memory_list_projection::{MemoryListQuery, MemoryListProjection};

/// Concrete projector executing over persisted database records.
pub struct SqliteProjector {
    storage: SqliteStorage,
}

impl SqliteProjector {
    /// Creates a new `SqliteProjector`.
    pub fn new(storage: SqliteStorage) -> Self {
        Self { storage }
    }
}

impl Projector<MemoryListProjection, MemoryListQuery> for SqliteProjector {
    fn project(&self, context: &ProjectionContext<MemoryListQuery>) -> MemoryListProjection {
        // Query the abstract repository interface
        // CONSTRAINT: Absolutely no SQL is written here.
        let nodes = match self.storage.run_transaction(|tx| {
            tx.repositories().nodes().list_all()
        }) {
            Ok(n) => n,
            Err(_) => Vec::new(),
        };

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
    storage: SqliteStorage,
    _epoch: Arc<Mutex<EpochId>>,
    _event_dispatcher: Arc<dyn RuntimeEventDispatcher>,
}

impl SqliteProjectionManager {
    /// Creates a new `SqliteProjectionManager`.
    pub fn new(
        storage: SqliteStorage,
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
        let current_epoch = match self.storage.run_transaction(|tx| {
            let epoch_str = tx.repositories().configs().get_key("current_epoch")?;
            let epoch = match epoch_str {
                Some(s) => {
                    let val = s.parse::<u64>().map_err(|e| brain_core::errors::BrainError::Storage {
                        message: format!("Failed to parse persisted epoch: {}", e),
                        source: None,
                    })?;
                    EpochId(val)
                }
                None => EpochId::initial(),
            };
            Ok(epoch)
        }) {
            Ok(e) => e,
            Err(_) => EpochId::initial(),
        };

        // Reconstruct the graph context snapshot from SQLite database transaction
        let mut graph = KnowledgeGraph::new();
        if let Ok(nodes) = self.storage.run_transaction(|tx| tx.repositories().nodes().list_all()) {
            for n in nodes {
                graph.nodes.insert(n.id, n);
            }
        }

        let context = ProjectionContext {
            graph: &graph,
            epoch: current_epoch,
            query,
            correlation_id,
        };

        projector.project(&context)
    }
}
