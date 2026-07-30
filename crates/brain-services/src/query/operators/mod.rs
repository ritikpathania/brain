//! Physical batch operators for pull-scheduled execution.

pub mod limit;
pub mod scan;

pub use limit::*;
pub use scan::*;

use crate::query::batch::*;
use crate::query::context::*;
use brain_domain::bkf::*;
use brain_domain::query::*;

/// Status returned after pulling a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchStatus {
    /// More batches available upstream.
    HaveMore,
    /// Upstream depleted.
    Exhausted,
}

/// Operator metrics container.
#[derive(Debug, Clone, Default)]
pub struct OperatorMetrics {
    /// Input rows.
    pub rows_in: usize,
    /// Output rows.
    pub rows_out: usize,
    /// Batches processed.
    pub batches: usize,
}

/// Pure physical operator interface.
pub trait PhysicalOperator: Send + Sync {
    /// Pulls next vectorized batch.
    fn next_batch(
        &mut self,
        snapshot: &dyn KnowledgeSnapshotView,
        config: &ExecutionConfig,
        state: &mut ExecutionState,
        output: &mut BindingBatch,
    ) -> Result<BatchStatus, QueryExecutionError>;

    /// Operator runtime metrics.
    fn metrics(&self) -> OperatorMetrics;
}
