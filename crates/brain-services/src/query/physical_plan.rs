//! Immutable physical plan representation.

use brain_domain::query::*;
use serde::{Deserialize, Serialize};

/// Physical plan representation node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicalPlanNode {
    /// Physical snapshot scan operator node over typed ScanTarget.
    Scan {
        /// Target scan view.
        target: ScanTarget,
    },
    /// Physical filter node.
    Filter {
        /// Filter description.
        description: String,
        /// Input node.
        input: Box<PhysicalPlanNode>,
    },
    /// Physical limit node.
    Limit {
        /// Count limit.
        count: usize,
        /// Input node.
        input: Box<PhysicalPlanNode>,
    },
}

/// Physical plan wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalPlan {
    /// Root physical node.
    pub root: PhysicalPlanNode,
}

impl PhysicalPlan {
    /// Returns name of root operator.
    pub fn root_name(&self) -> &'static str {
        match &self.root {
            PhysicalPlanNode::Scan { .. } => "PhysicalScan",
            PhysicalPlanNode::Filter { .. } => "PhysicalFilter",
            PhysicalPlanNode::Limit { .. } => "PhysicalLimit",
        }
    }
}
