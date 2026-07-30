//! Projection instance container holding reducer, lifecycle state, checkpoint, and telemetry metrics.

use brain_domain::bkf::events::FactEvent;
use brain_domain::bkf::Timestamp;
use brain_domain::projection::*;
use serde::{Deserialize, Serialize};

/// Explicit lifecycle states of a projection instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionLifecycle {
    /// Registered in runtime but not initialized.
    Registered,
    /// Loading checkpoint or preparing storage.
    Initializing,
    /// Performing catch-up event replay.
    Replaying,
    /// Processing live event stream.
    Live,
    /// Gracefully stopping.
    Stopping,
    /// Terminated/stopped.
    Stopped,
}

/// Dedicated runtime metrics for projection instance.
#[derive(Debug, Clone, Default)]
pub struct ProjectionMetrics {
    /// Total events processed.
    pub events_processed: u64,
}

/// Container wrapping a projection reducer alongside runtime metadata.
pub struct ProjectionInstance {
    reducer: Box<dyn ProjectionReducer>,
    lifecycle: ProjectionLifecycle,
    checkpoint: Checkpoint,
    metrics: ProjectionMetrics,
}

impl ProjectionInstance {
    /// Creates a new ProjectionInstance around a reducer.
    pub fn new(reducer: Box<dyn ProjectionReducer>) -> Self {
        let id = reducer.id();
        let version = reducer.version();
        Self {
            reducer,
            lifecycle: ProjectionLifecycle::Registered,
            checkpoint: Checkpoint {
                projection_id: id,
                version,
                watermark: Watermark(0),
                timestamp: Timestamp::now(),
                state_hash: None,
            },
            metrics: ProjectionMetrics::default(),
        }
    }

    /// Returns projection ID.
    pub fn id(&self) -> ProjectionId {
        self.reducer.id()
    }

    /// Returns projection version.
    pub fn version(&self) -> ProjectionVersion {
        self.reducer.version()
    }

    /// Returns current lifecycle state.
    pub fn lifecycle(&self) -> ProjectionLifecycle {
        self.lifecycle
    }

    /// Sets lifecycle state.
    pub fn set_lifecycle(&mut self, state: ProjectionLifecycle) {
        self.lifecycle = state;
    }

    /// Returns current checkpoint.
    pub fn checkpoint(&self) -> &Checkpoint {
        &self.checkpoint
    }

    /// Returns runtime metrics.
    pub fn metrics(&self) -> &ProjectionMetrics {
        &self.metrics
    }

    /// Applies domain event and updates watermark.
    pub fn apply_event(&mut self, event: &FactEvent, seq: u64) -> Result<(), ProjectionError> {
        self.reducer.apply_event(event)?;
        self.checkpoint.watermark = Watermark(seq);
        self.metrics.events_processed += 1;
        Ok(())
    }
}
