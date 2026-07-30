//! Physical limit operator.

use crate::query::operators::*;
use brain_domain::bkf::*;
use brain_domain::query::*;

/// Physical operator truncating upstream batches to a maximum limit count.
pub struct LimitOperator {
    limit: usize,
    emitted: usize,
    input: Box<dyn PhysicalOperator>,
    metrics: OperatorMetrics,
}

impl LimitOperator {
    /// Creates a new LimitOperator.
    pub fn new(limit: usize, input: Box<dyn PhysicalOperator>) -> Self {
        Self {
            limit,
            emitted: 0,
            input,
            metrics: OperatorMetrics::default(),
        }
    }
}

impl PhysicalOperator for LimitOperator {
    fn next_batch(
        &mut self,
        snapshot: &dyn KnowledgeSnapshotView,
        config: &ExecutionConfig,
        state: &mut ExecutionState,
        output: &mut BindingBatch,
    ) -> Result<BatchStatus, QueryExecutionError> {
        if self.emitted >= self.limit {
            return Ok(BatchStatus::Exhausted);
        }

        let status = self.input.next_batch(snapshot, config, state, output)?;
        self.metrics.rows_in += output.len();

        if output.len() + self.emitted > self.limit {
            let keep = self.limit - self.emitted;
            output.truncate(keep);
        }

        self.emitted += output.len();
        self.metrics.rows_out = self.emitted;
        self.metrics.batches += 1;

        if self.emitted >= self.limit {
            Ok(BatchStatus::Exhausted)
        } else {
            Ok(status)
        }
    }

    fn metrics(&self) -> OperatorMetrics {
        self.metrics.clone()
    }
}
