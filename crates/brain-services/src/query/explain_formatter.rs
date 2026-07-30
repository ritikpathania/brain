//! Explain plan formatter.

use crate::query::physical_plan::*;
use brain_domain::query::*;

/// Formatter for EXPLAIN command outputs.
pub struct ExplainFormatter;

impl ExplainFormatter {
    /// Formats logical and physical plans into ExplainPlan value object.
    pub fn format(logical: &LogicalPlan, physical: &PhysicalPlan) -> ExplainPlan {
        ExplainPlan {
            logical_plan_str: format!("{:#?}", logical),
            physical_plan_str: format!("{:#?}", physical),
        }
    }
}
