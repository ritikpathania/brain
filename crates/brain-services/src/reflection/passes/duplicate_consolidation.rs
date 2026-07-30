//! Duplicate consolidation pass for Reflection Engine v2.

use crate::reflection::pass_context::*;
use brain_domain::bkf::*;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Reflection v2 pass detecting duplicate active facts referencing the exact same assertion ID and consolidating them.
pub struct V2DuplicateConsolidationPass;

impl V2ReflectionPass for V2DuplicateConsolidationPass {
    fn id(&self) -> PassId {
        PassId::new("duplicate_consolidation")
    }

    fn dependencies(&self) -> &[PassId] {
        static DEPS: OnceLock<Vec<PassId>> = OnceLock::new();
        DEPS.get_or_init(|| {
            vec![
                PassId::new("canonicalization"),
                PassId::new("contradiction"),
            ]
        })
    }

    fn analyze(
        &self,
        snapshot: &dyn KnowledgeSnapshotView,
        _context: &V2ReflectionContext,
    ) -> Result<Option<ReflectionOutcome>, String> {
        // Group active facts by assertion_id
        let mut grouped: HashMap<AssertionId, Vec<&FactVersion>> = HashMap::new();
        for fact in snapshot.active_facts() {
            grouped.entry(fact.assertion_id).or_default().push(fact);
        }

        let mut ops = Vec::new();
        let mut diagnostics = Vec::new();

        for (assertion_id, mut facts) in grouped {
            if facts.len() <= 1 {
                continue;
            }

            // Sort by fact ID string for deterministic canonical target selection
            facts.sort_by_key(|a| a.id.0.to_string());

            let target = facts[0];
            let sources: Vec<FactVersionId> = facts[1..].iter().map(|f| f.id).collect();

            diagnostics.push(PassDiagnostic {
                severity: DiagnosticSeverity::Info,
                code: "DUPLICATE_FACTS_MERGED".to_string(),
                message: format!(
                    "Consolidating {} duplicate facts for assertion {} into target fact {}",
                    sources.len(),
                    assertion_id.0,
                    target.id.0
                ),
            });

            ops.push(RewriteOperation::MergeFacts {
                source_fact_ids: sources,
                target_fact_id: target.id,
            });
        }

        if ops.is_empty() {
            return Ok(None);
        }

        let plan = RewritePlan {
            pass_id: self.id(),
            reason: RewriteReason::Duplicate,
            rationale: format!("Consolidated {} duplicate fact clusters", ops.len()),
            execution_cost: ops.len() as u32,
            operations: ops,
        };

        Ok(Some(ReflectionOutcome { plan, diagnostics }))
    }
}
