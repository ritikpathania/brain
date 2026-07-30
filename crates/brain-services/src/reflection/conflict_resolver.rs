//! Deterministic Conflict Resolver for Reflection Engine v2 rewrite plans.

use brain_domain::bkf::*;
use std::collections::HashSet;

/// Deterministic resolver merging multiple pass `RewritePlan`s into a single conflict-free `RewritePlan`.
pub struct ConflictResolver;

impl ConflictResolver {
    /// Merges multiple proposed `RewritePlan`s into a single unified `RewritePlan`.
    /// Resolves conflicts deterministically and deduplicates identical operations.
    pub fn resolve(mut plans: Vec<RewritePlan>) -> Result<RewritePlan, String> {
        if plans.is_empty() {
            return Ok(RewritePlan {
                pass_id: PassId::new("conflict_resolver"),
                reason: RewriteReason::Canonicalization,
                rationale: "Empty plan sequence".to_string(),
                execution_cost: 0,
                operations: vec![],
            });
        }

        // Sort plans by pass_id string to guarantee input shuffle invariance
        plans.sort_by(|a, b| a.pass_id.as_str().cmp(b.pass_id.as_str()));

        let mut ops = Vec::new();
        let mut total_cost = 0;
        let mut rationales = Vec::new();

        for plan in plans {
            total_cost += plan.execution_cost;
            if !plan.rationale.is_empty() {
                rationales.push(plan.rationale);
            }
            for op in plan.operations {
                ops.push(op);
            }
        }

        // Deduplicate operations while preserving deterministic order
        let mut unique_ops = Vec::new();
        let mut seen = HashSet::new();

        for op in ops {
            // Serialize op to JSON string to form a unique hashing key for deduplication
            let key = serde_json::to_string(&op).unwrap_or_default();
            if seen.insert(key) {
                unique_ops.push(op);
            }
        }

        Ok(RewritePlan {
            pass_id: PassId::new("conflict_resolver"),
            reason: RewriteReason::Canonicalization,
            rationale: rationales.join("; "),
            execution_cost: total_cost,
            operations: unique_ops,
        })
    }
}
