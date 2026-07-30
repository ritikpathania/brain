//! Phase 3 Projection Runtime (single-writer, catch-up replay, atomic checkpoints, graceful shutdown).

/// Projection instance container holding reducer, lifecycle state, checkpoint, and metrics.
pub mod instance;
pub use instance::*;
