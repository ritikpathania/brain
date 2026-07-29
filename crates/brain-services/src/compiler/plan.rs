//! Immutable, static CompilerExecutionPlan for deterministic compiler pass execution.

use crate::compiler::telemetry::PassId;

/// Immutable execution plan defining a static, deterministic sequence of compiler passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerExecutionPlan {
    /// Ordered list of pass IDs to execute.
    passes: Vec<PassId>,
}

impl Default for CompilerExecutionPlan {
    fn default() -> Self {
        Self::standard_3tier_pipeline()
    }
}

impl CompilerExecutionPlan {
    /// Instantiates the standard 3-tier compiler execution plan topology.
    pub fn standard_3tier_pipeline() -> Self {
        Self {
            passes: vec![
                // 1. Front End Tier
                PassId::ObservationNormalization,
                PassId::Validation,
                // 2. Middle End Tier
                PassId::CanonicalEntityResolution,
                PassId::ConfidenceAggregation,
                PassId::TemporalFactResolution,
                PassId::CompilerContradiction,
                // 3. Back End Tier
                PassId::ProvenanceMerge,
                PassId::FactDeduplication,
                PassId::RelationDeduplication,
            ],
        }
    }

    /// Returns a slice of the ordered pass IDs in this execution plan.
    pub fn passes(&self) -> &[PassId] {
        &self.passes
    }

    /// Executes the plan's pass sequence over a mutable CompilerState, recording telemetry and syncing the target graph.
    pub fn execute(&self, state: &mut crate::compiler::state::CompilerState) {
        for &pass_id in &self.passes {
            let start = std::time::Instant::now();
            let initial_diagnostics = state.diagnostics.len();

            // Execute pure pass transformation based on pass_id
            match pass_id {
                PassId::ObservationNormalization => {
                    // Standardize entity names & trim whitespace
                    for entity in state.ir_after.entities.values_mut() {
                        entity.canonical_name = entity.canonical_name.trim().to_string();
                    }
                }
                PassId::Validation => {
                    // Validate entity & fact invariants
                    for entity in state.ir_after.entities.values() {
                        if entity.canonical_name.is_empty() {
                            state
                                .diagnostics
                                .push(crate::compiler::diagnostics::Diagnostic::new(
                                    crate::compiler::diagnostics::DiagnosticLevel::Error,
                                    crate::compiler::diagnostics::DiagnosticKind::AmbiguousIdentity,
                                    entity.id.0.clone(),
                                    "Entity canonical name cannot be empty",
                                ));
                        }
                    }
                }
                _ => {}
            }

            let duration_ns = start.elapsed().as_nanos() as u64;
            let emitted = state.diagnostics.len() - initial_diagnostics;
            state.trace.record_pass(pass_id, emitted, duration_ns);
        }

        // Synchronize canonical graph state after pass execution
        state.sync_graph_after();
    }
}
