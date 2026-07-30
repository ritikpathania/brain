//! Confidence recalculation pass for evaluating lineage depth and source corroboration in Reflection Engine v2.

use crate::reflection::pass_context::*;
use brain_domain::bkf::*;
use std::sync::OnceLock;
use uuid::Uuid;

/// Reflection v2 pass evaluating fact provenance lineage and updating confidence scores.
pub struct V2ConfidenceRecalculationPass;

impl V2ReflectionPass for V2ConfidenceRecalculationPass {
    fn id(&self) -> PassId {
        PassId::new("confidence_recalculation")
    }

    fn dependencies(&self) -> &[PassId] {
        static DEPS: OnceLock<Vec<PassId>> = OnceLock::new();
        DEPS.get_or_init(|| {
            vec![
                PassId::new("canonicalization"),
                PassId::new("contradiction"),
                PassId::new("stale_knowledge"),
            ]
        })
    }

    fn analyze(
        &self,
        snapshot: &dyn KnowledgeSnapshotView,
        _context: &V2ReflectionContext,
    ) -> Result<Option<ReflectionOutcome>, String> {
        let mut ops = Vec::new();
        let mut diagnostics = Vec::new();

        for fact in snapshot.active_facts() {
            // Corroboration boost calculation: derived facts with >= 2 sources get boosted
            let lineage_count = fact.provenance.derived_from.len();
            if lineage_count >= 2 {
                let boost = 0.2_f32;
                let new_score = (fact.confidence.value() + boost).min(1.0);

                if (new_score - fact.confidence.value()).abs() >= 0.05 {
                    let mut updated_fact = fact.clone();
                    updated_fact.id = FactVersionId(Uuid::new_v4());
                    updated_fact.confidence = Confidence::new(new_score)?;
                    updated_fact.supersedes = Some(fact.id);

                    diagnostics.push(PassDiagnostic {
                        severity: DiagnosticSeverity::Info,
                        code: "CONFIDENCE_BOOSTED".to_string(),
                        message: format!(
                            "Fact {} confidence boosted from {:.2} to {:.2} due to {} corroborating sources",
                            fact.id.0, fact.confidence.value(), new_score, lineage_count
                        ),
                    });

                    ops.push(RewriteOperation::RecordFact(updated_fact));
                }
            }
        }

        if ops.is_empty() {
            return Ok(None);
        }

        let plan = RewritePlan {
            pass_id: self.id(),
            reason: RewriteReason::ConfidenceIncrease,
            rationale: format!(
                "Recalculated confidence for {} corroborated facts",
                ops.len()
            ),
            execution_cost: ops.len() as u32,
            operations: ops,
        };

        Ok(Some(ReflectionOutcome { plan, diagnostics }))
    }
}
