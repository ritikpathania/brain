//! Typed error hierarchy for projection runtime.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error during projection reduction or replay.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum ProjectionError {
    /// Reducer execution failed.
    #[error("Reducer error: {message}")]
    ReducerFailed {
        /// Error details.
        message: String,
    },
    /// Version mismatch between state and code.
    #[error("Version mismatch: expected {expected}, found {found}")]
    VersionMismatch {
        /// Expected version.
        expected: u32,
        /// Found version.
        found: u32,
    },
    /// Checkpoint corrupted.
    #[error("Checkpoint corrupted: {detail}")]
    CheckpointCorrupted {
        /// Error detail.
        detail: String,
    },
    /// Catch-up replay failed.
    #[error("Replay failed at watermark {watermark}: {reason}")]
    ReplayFailed {
        /// Watermark offset.
        watermark: u64,
        /// Failure reason.
        reason: String,
    },
}
