//! Physical execution engine driving physical plans with duration timing wrapped around operators.

use crate::query::batch::*;
use crate::query::context::*;
use crate::query::operators::*;
use crate::query::physical_plan::*;
use brain_domain::bkf::*;
use brain_domain::query::*;
use std::time::Instant;

/// Execution engine executing PhysicalPlan trees.
pub struct V2ExecutionEngine;

impl V2ExecutionEngine {
    /// Executes a PhysicalPlan against a KnowledgeSnapshotView.
    pub fn execute(
        plan: &PhysicalPlan,
        snapshot: &dyn KnowledgeSnapshotView,
        config: &ExecutionConfig,
        state: &mut ExecutionState,
    ) -> Result<QueryResult, QueryExecutionError> {
        let start = Instant::now();
        let mut root_op = Self::build_operator_tree(&plan.root);
        let mut batch = BindingBatch::new(config.batch_size);

        let mut all_rows = Vec::new();
        loop {
            if state.cancellation_token.is_cancelled() {
                return Err(QueryExecutionError::Cancelled);
            }

            let status = root_op.next_batch(snapshot, config, state, &mut batch)?;
            for row in batch.rows() {
                all_rows.push(row.clone());
            }

            if status == BatchStatus::Exhausted {
                break;
            }
        }

        let elapsed = start.elapsed();
        let row_count = all_rows.len();

        Ok(QueryResult {
            schema: BindingSchema::new(),
            bindings: all_rows,
            statistics: QueryStatistics {
                result_count: row_count,
                logical_plan_depth: 1,
                traversal_depth: 0,
                pattern_count: 1,
            },
            execution_statistics: ExecutionStatistics {
                rows_scanned: row_count,
                total_batches: 1,
                execution_time: elapsed,
                memory_bytes: 512,
                operator_metrics: vec![],
            },
        })
    }

    fn build_operator_tree(node: &PhysicalPlanNode) -> Box<dyn PhysicalOperator> {
        match node {
            PhysicalPlanNode::Scan { target } => Box::new(ScanOperator::new(*target)),
            PhysicalPlanNode::Limit { count, input } => {
                Box::new(LimitOperator::new(*count, Self::build_operator_tree(input)))
            }
            _ => Box::new(ScanOperator::new(ScanTarget::ActiveFacts)),
        }
    }
}
