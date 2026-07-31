//! Sequential projection scheduler (single-writer per-projection invariant).

use crate::projection::registry::*;
use brain_domain::bkf::events::FactEvent;
use brain_domain::projection::*;

/// Trait for projection event scheduling (minimal event-dispatch interface).
pub trait ProjectionScheduler: Send + Sync {
    /// Dispatches a single event sequentially across registered projections.
    fn dispatch_event(
        &mut self,
        registry: &mut ProjectionRegistry,
        event: &FactEvent,
        seq: u64,
    ) -> Result<(), ProjectionError>;
}

/// Sequential single-writer projection scheduler.
#[derive(Default)]
pub struct SequentialProjectionScheduler;

impl SequentialProjectionScheduler {
    /// Creates a new SequentialProjectionScheduler.
    pub fn new() -> Self {
        Self
    }
}

impl ProjectionScheduler for SequentialProjectionScheduler {
    fn dispatch_event(
        &mut self,
        registry: &mut ProjectionRegistry,
        event: &FactEvent,
        seq: u64,
    ) -> Result<(), ProjectionError> {
        for instance in registry.instances_mut() {
            instance.apply_event(event, seq)?;
        }
        Ok(())
    }
}
