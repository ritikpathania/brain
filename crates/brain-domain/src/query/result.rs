//! Query result and statistics value objects with slot-indexed binding rows.

use crate::bkf::*;
use crate::query::bound::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Value bound to a query variable slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryValue {
    /// Entity reference.
    Entity(KnowledgeEntity),
    /// Fact version reference.
    Fact(FactVersion),
    /// Scalar literal.
    Literal(LiteralValue),
}

/// Slot-indexed binding row vector.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BindingRow {
    /// Compact slot-indexed vector.
    pub slots: Vec<Option<QueryValue>>,
}

impl BindingRow {
    /// Creates a new BindingRow with slot capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
        }
    }

    /// Sets a slot value.
    pub fn set(&mut self, slot: SlotId, val: QueryValue) {
        if slot.0 >= self.slots.len() {
            self.slots.resize(slot.0 + 1, None);
        }
        self.slots[slot.0] = Some(val);
    }

    /// Gets a slot value.
    pub fn get(&self, slot: SlotId) -> Option<&QueryValue> {
        self.slots.get(slot.0).and_then(|v| v.as_ref())
    }
}

/// Logical statistics for query results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryStatistics {
    /// Total result rows returned.
    pub result_count: usize,
    /// Logical plan tree depth.
    pub logical_plan_depth: usize,
    /// Traversal depth expanded.
    pub traversal_depth: usize,
    /// Total pattern rules matched.
    pub pattern_count: usize,
}

/// Operator metric entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorMetricEntry {
    /// Operator identifier.
    pub operator_name: String,
    /// Input rows.
    pub rows_in: usize,
    /// Output rows.
    pub rows_out: usize,
    /// Batches processed.
    pub batches: usize,
}

/// Execution telemetry statistics.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionStatistics {
    /// Total facts scanned from snapshot.
    pub rows_scanned: usize,
    /// Total batches processed.
    pub total_batches: usize,
    /// Total execution duration.
    pub execution_time: Duration,
    /// Peak memory allocation bytes.
    pub memory_bytes: usize,
    /// Per-operator runtime metrics.
    pub operator_metrics: Vec<OperatorMetricEntry>,
}

/// Complete query execution result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    /// Schema mapping variables to slot IDs.
    pub schema: BindingSchema,
    /// Binding rows.
    pub bindings: Vec<BindingRow>,
    /// Logical statistics.
    pub statistics: QueryStatistics,
    /// Telemetry statistics.
    pub execution_statistics: ExecutionStatistics,
}
