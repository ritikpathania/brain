//! Pure domain projection reducer contract.

use crate::bkf::events::FactEvent;
use crate::projection::errors::*;
use crate::projection::id::*;

/// Core domain reducer trait processing events (replay transparent).
pub trait ProjectionReducer: Send + Sync {
    /// Unique identifier for projection.
    fn id(&self) -> ProjectionId;
    /// Schema/code version of projection logic.
    fn version(&self) -> ProjectionVersion;
    /// Applies a domain event to update internal state.
    fn apply_event(&mut self, event: &FactEvent) -> Result<(), ProjectionError>;
    /// Resets projection state back to initial/empty conditions.
    fn reset(&mut self) -> Result<(), ProjectionError>;
}
