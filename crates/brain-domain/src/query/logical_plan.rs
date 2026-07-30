//! Immutable logical algebra nodes.

use crate::query::filters::*;
use crate::query::scan_target::*;
use serde::{Deserialize, Serialize};

/// Logical plan algebra nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogicalPlan {
    /// Data source scan over typed ScanTarget.
    Scan {
        /// Target scan entity/fact view.
        target: ScanTarget,
    },
    /// Logical predicate filter.
    Filter {
        /// Expression condition.
        condition: QueryFilter,
        /// Child input plan.
        input: Box<LogicalPlan>,
    },
    /// Logical pattern join.
    Join {
        /// Left input plan.
        left: Box<LogicalPlan>,
        /// Right input plan.
        right: Box<LogicalPlan>,
    },
    /// Logical graph traversal.
    Traverse {
        /// Max hop depth.
        max_depth: u32,
        /// Child input plan.
        input: Box<LogicalPlan>,
    },
    /// Limit truncation.
    Limit {
        /// Maximum row count.
        count: usize,
        /// Child input plan.
        input: Box<LogicalPlan>,
    },
}
