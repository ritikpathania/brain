//! Stale knowledge reflection pass for archiving expired temporal window facts in Reflection Engine v2.

use crate::reflection::pass_context::*;
use brain_domain::bkf::*;
use std::sync::OnceLock;

/// Reflection v2 pass scanning active facts for expired temporal windows and proposing archiving.
pub struct V2StaleKnowledgePass;

impl V2ReflectionPass for V2StaleKnowledgePass {
    fn id(&self) -> PassId {
        PassId::new("stale_knowledge")
    }

    fn dependencies(&self) -> &[PassId] {
        static DEPS: OnceLock<Vec<PassId>> = OnceLock::new();
        DEPS.get_or_init(|| {
            vec![
                PassId::new("canonicalization"),
                PassId::new("contradiction"),
                PassId::new("duplicate_consolidation"),
            ]
        })
    }

    fn analyze(
        &self,
        snapshot: &dyn KnowledgeSnapshotView,
        context: &V2ReflectionContext,
    ) -> Result<Option<ReflectionOutcome>, String> {
        let mut ops = Vec::new();
        let mut diagnostics = Vec::new();

        for fact in snapshot.active_facts() {
            if let Some(valid_to) = fact.temporal.valid_to {
                if valid_to <= context.now {
                    diagnostics.push(PassDiagnostic {
                        severity: DiagnosticSeverity::Info,
                        code: "FACT_TEMPORALLY_EXPIRED".to_string(),
                        message: format!(
                            "Fact {} valid_to timestamp {:?} expired at context time {:?}",
                            fact.id.0, valid_to, context.now
                        ),
                    });

                    ops.push(RewriteOperation::ArchiveFact {
                        fact_id: fact.id,
                        archived_at: context.now,
                    });
                }
            }
        }

        if ops.is_empty() {
            return Ok(None);
        }

        let plan = RewritePlan {
            pass_id: self.id(),
            reason: RewriteReason::TemporalExpiration,
            rationale: format!("Archived {} temporally expired facts", ops.len()),
            execution_cost: ops.len() as u32,
            operations: ops,
        };

        Ok(Some(ReflectionOutcome { plan, diagnostics }))
    }
}
