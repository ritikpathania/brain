//! Phase 3 Projection Runtime (single-writer, catch-up replay, atomic checkpoints, graceful shutdown).

/// Projection instance container holding reducer, lifecycle state, checkpoint, and metrics.
pub mod instance;
pub use instance::*;
/// Projection registry.
pub mod registry;
pub use registry::*;
/// Storage-agnostic CheckpointStore trait and InMemoryCheckpointStore.
pub mod store;
pub use store::*;
