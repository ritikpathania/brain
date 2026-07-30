//! Formatted EXPLAIN query plan output.

use serde::{Deserialize, Serialize};

/// Formatted logical and physical query plans for EXPLAIN commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainPlan {
    /// Formatted string of the logical plan.
    pub logical_plan_str: String,
    /// Formatted string of the physical plan.
    pub physical_plan_str: String,
}
