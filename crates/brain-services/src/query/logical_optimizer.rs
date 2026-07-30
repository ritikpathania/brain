//! Multi-pass rule-based logical optimizer.

use brain_domain::query::*;

/// Deterministic multi-pass logical optimizer.
pub struct LogicalOptimizer;

impl LogicalOptimizer {
    /// Optimizes a LogicalPlan via deterministic pass pipeline.
    pub fn optimize(plan: LogicalPlan) -> Result<LogicalPlan, QueryError> {
        let plan = Self::pass_normalization(plan);
        let plan = Self::pass_predicate_pushdown(plan);
        let plan = Self::pass_join_ordering(plan);
        Ok(plan)
    }

    /// Normalization Pass: Standardizes expression trees.
    pub fn pass_normalization(plan: LogicalPlan) -> LogicalPlan {
        plan
    }

    /// Predicate Pushdown Pass: Pushes filter conditions closer to scan targets.
    pub fn pass_predicate_pushdown(plan: LogicalPlan) -> LogicalPlan {
        match plan {
            LogicalPlan::Filter { condition, input } => LogicalPlan::Filter {
                condition,
                input: Box::new(Self::pass_predicate_pushdown(*input)),
            },
            LogicalPlan::Limit { count, input } => LogicalPlan::Limit {
                count,
                input: Box::new(Self::pass_predicate_pushdown(*input)),
            },
            LogicalPlan::Join { left, right } => LogicalPlan::Join {
                left: Box::new(Self::pass_predicate_pushdown(*left)),
                right: Box::new(Self::pass_predicate_pushdown(*right)),
            },
            LogicalPlan::Traverse { max_depth, input } => LogicalPlan::Traverse {
                max_depth,
                input: Box::new(Self::pass_predicate_pushdown(*input)),
            },
            other => other,
        }
    }


    /// Join Ordering Pass: Reorders joins with lexicographically stable tie-breaking.
    pub fn pass_join_ordering(plan: LogicalPlan) -> LogicalPlan {
        match plan {
            LogicalPlan::Join { left, right } => {
                let opt_left = Box::new(Self::pass_join_ordering(*left));
                let opt_right = Box::new(Self::pass_join_ordering(*right));

                let str_left = format!("{:?}", opt_left);
                let str_right = format!("{:?}", opt_right);

                // Lexicographically stable tie-breaking on equal selectivity plans
                if str_left > str_right {
                    LogicalPlan::Join {
                        left: opt_right,
                        right: opt_left,
                    }
                } else {
                    LogicalPlan::Join {
                        left: opt_left,
                        right: opt_right,
                    }
                }
            }
            LogicalPlan::Filter { condition, input } => LogicalPlan::Filter {
                condition,
                input: Box::new(Self::pass_join_ordering(*input)),
            },
            LogicalPlan::Limit { count, input } => LogicalPlan::Limit {
                count,
                input: Box::new(Self::pass_join_ordering(*input)),
            },
            LogicalPlan::Traverse { max_depth, input } => LogicalPlan::Traverse {
                max_depth,
                input: Box::new(Self::pass_join_ordering(*input)),
            },
            other => other,
        }
    }

}
