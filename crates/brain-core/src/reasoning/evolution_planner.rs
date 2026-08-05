//! EvolutionPlannerService deriving declarative EvolutionPlans strictly from ReflectionReports.

use brain_domain::{
    DomainError, EvolutionAction, EvolutionActionId, EvolutionPlan, ReflectionFindingKind,
    ReflectionReport,
};

/// Pure domain service deriving declarative evolution action plans from ReflectionReports.
///
/// Invariants:
/// - EvolutionPlannerService depends strictly on ReflectionReport; it never accesses ArtifactStore or ExecutionState directly.
/// - EvolutionPlannerService proposes actions declaratively; it performs no memory mutations itself.
/// - Given identical ReflectionReports, EvolutionPlannerService produces identical EvolutionPlans (idempotency).
#[derive(Debug, Clone, Default)]
pub struct EvolutionPlannerService;

impl EvolutionPlannerService {
    /// Instantiates a new `EvolutionPlannerService`.
    pub fn new() -> Self {
        Self
    }

    /// Derives an `EvolutionPlan` strictly from a `ReflectionReport`.
    pub fn plan_evolution(&self, report: &ReflectionReport) -> Result<EvolutionPlan, DomainError> {
        let mut actions = Vec::new();

        for finding in &report.findings {
            match finding.kind {
                ReflectionFindingKind::MissingEvidence
                | ReflectionFindingKind::IncompleteReasoning => {
                    if let Some(first_art_id) = finding.supporting_evidence.iter().next() {
                        actions.push(EvolutionAction::MarkConflict {
                            id: EvolutionActionId::new(),
                            artifact_id: *first_art_id,
                        });
                    }
                }
                ReflectionFindingKind::Contradiction => {
                    for art_id in finding.supporting_evidence.iter() {
                        actions.push(EvolutionAction::MarkConflict {
                            id: EvolutionActionId::new(),
                            artifact_id: *art_id,
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(EvolutionPlan::new(report.execution_id, actions))
    }
}
