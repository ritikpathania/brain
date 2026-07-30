//! Canonicalization reflection pass for normalizing casing, whitespace, and entity names.

use crate::reflection::pass_context::*;
use brain_domain::bkf::*;

/// Reflection pass that analyzes text casing and whitespace formatting across active entities.
pub struct CanonicalizationPass;

impl V2ReflectionPass for CanonicalizationPass {
    fn id(&self) -> PassId {
        PassId::new("canonicalization")
    }

    fn dependencies(&self) -> &[PassId] {
        &[]
    }

    fn analyze(
        &self,
        snapshot: &dyn KnowledgeSnapshotView,
        _context: &V2ReflectionContext,
    ) -> Result<Option<ReflectionOutcome>, String> {
        let ops = Vec::new();
        let mut diagnostics = Vec::new();

        for entity in snapshot.entities() {
            let raw_name = entity.name.as_str();
            let normalized = raw_name.trim();

            // Capitalize first letter if lowercased
            let mut chars = normalized.chars();
            let canonical_name = match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            };

            if canonical_name != raw_name {
                diagnostics.push(PassDiagnostic {
                    severity: DiagnosticSeverity::Info,
                    code: "CANONICALIZED_NAME".to_string(),
                    message: format!("Normalized entity '{}' -> '{}'", raw_name, canonical_name),
                });
            }
        }

        if diagnostics.is_empty() {
            return Ok(None);
        }

        let plan = RewritePlan {
            pass_id: self.id(),
            reason: RewriteReason::Canonicalization,
            rationale: format!("Canonicalized {} entity names", diagnostics.len()),
            execution_cost: ops.len() as u32,
            operations: ops,
        };

        Ok(Some(ReflectionOutcome { plan, diagnostics }))
    }
}
