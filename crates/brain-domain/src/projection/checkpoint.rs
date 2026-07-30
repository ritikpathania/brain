//! Immutable projection checkpoint value object.

use crate::bkf::Timestamp;
use crate::projection::id::*;
use crate::projection::watermark::*;
use serde::{Deserialize, Serialize};

/// Immutable checkpoint record tracking watermark position and state hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Target projection ID.
    pub projection_id: ProjectionId,
    /// Projection code/schema version.
    pub version: ProjectionVersion,
    /// Current sequence watermark.
    pub watermark: Watermark,
    /// Checkpoint timestamp.
    pub timestamp: Timestamp,
    /// Optional state hash for verification.
    pub state_hash: Option<String>,
}
