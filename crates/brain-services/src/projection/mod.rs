//! Phase 3 & 4 Projection Runtime & Read Models (single-writer, catch-up replay, atomic checkpoints, graceful shutdown).

/// Projection instance container holding reducer, lifecycle state, checkpoint, and metrics.
pub mod instance;
pub use instance::*;
/// Projection registry.
pub mod registry;
pub use registry::*;
/// Storage-agnostic CheckpointStore trait and InMemoryCheckpointStore.
pub mod store;
pub use store::*;
/// Catch-up replay engine.
pub mod replay;
pub use replay::*;
/// Sequential projection scheduler.
pub mod scheduler;
pub use scheduler::*;
/// Projection runtime facade.
pub mod runtime;
pub use runtime::*;

/// Re-export Phase 4 domain projections.
pub use brain_domain::projection::graph_adjacency;
