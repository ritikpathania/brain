//! Physical scan operator.

use crate::query::operators::*;
use brain_domain::bkf::*;
use brain_domain::query::*;

/// Physical operator scanning facts/entities from snapshot over typed ScanTarget.
pub struct ScanOperator {
    target: ScanTarget,
    scanned: bool,
    metrics: OperatorMetrics,
}

impl ScanOperator {
    /// Creates a new ScanOperator for target.
    pub fn new(target: ScanTarget) -> Self {
        Self {
            target,
            scanned: false,
            metrics: OperatorMetrics::default(),
        }
    }
}

impl PhysicalOperator for ScanOperator {
    fn next_batch(
        &mut self,
        snapshot: &dyn KnowledgeSnapshotView,
        _config: &ExecutionConfig,
        _state: &mut ExecutionState,
        output: &mut BindingBatch,
    ) -> Result<BatchStatus, QueryExecutionError> {
        if self.scanned {
            return Ok(BatchStatus::Exhausted);
        }

        output.clear();
        if self.target == ScanTarget::ActiveFacts {
            for fact in snapshot.active_facts() {
                let mut row = BindingRow::with_capacity(1);
                row.set(SlotId(0), QueryValue::Fact(fact.clone()));
                output.append(row);
                self.metrics.rows_in += 1;
            }
        } else if self.target == ScanTarget::Entities {
            for entity in snapshot.entities() {
                let mut row = BindingRow::with_capacity(1);
                row.set(SlotId(0), QueryValue::Entity(entity.clone()));
                output.append(row);
                self.metrics.rows_in += 1;
            }
        }

        self.scanned = true;
        self.metrics.rows_out = output.len();
        self.metrics.batches += 1;
        Ok(BatchStatus::Exhausted)
    }

    fn metrics(&self) -> OperatorMetrics {
        self.metrics.clone()
    }
}
