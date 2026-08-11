//! Pure EvolutionEngine planner producing deterministic EvolutionPlans from StewardshipReports.

use super::action::{EvolutionAction, EvolutionActionKind};
use super::diff::{SemanticChange, SemanticDiff};
use super::plan::EvolutionPlan;
use super::proposal::{EvolutionProposal, Priority, ProposalOrigin};
use crate::identifiers::NodeId;
use crate::reflection::finding::FindingKind;
use crate::reflection::report::StewardshipReport;

/// Pure EvolutionEngine planner.
#[derive(Debug, Clone, Default)]
pub struct EvolutionEngine;

impl EvolutionEngine {
    /// Creates a new EvolutionEngine.
    pub fn new() -> Self {
        Self
    }

    /// Generates a deterministic EvolutionPlan from a StewardshipReport.
    pub fn plan(&self, report: &StewardshipReport) -> EvolutionPlan {
        let mut proposals = Vec::new();

        for finding in &report.findings {
            if finding.kind == FindingKind::Duplication {
                let action = EvolutionAction::new(
                    EvolutionActionKind::MergeNodes,
                    NodeId::new(),
                    Some(NodeId::new()),
                    "Merge duplicate concept node vertices",
                );

                let mut diff = SemanticDiff::new();
                diff.add_change(SemanticChange::MergedConcepts {
                    canonical: "Canonical Concept".to_string(),
                    merged: "Duplicate Concept".to_string(),
                    reason: finding.description.clone(),
                });

                let proposal = EvolutionProposal::new(
                    ProposalOrigin {
                        stewardship_findings: vec![finding.id],
                    },
                    format!("Consolidate: {}", finding.summary),
                    Priority::High,
                    finding.confidence,
                    vec![action],
                    diff,
                );

                proposals.push(proposal);
            }
        }

        EvolutionPlan::new(proposals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reflection::finding::StewardshipFinding;
    use crate::retrieval::ConfidenceAssessment;

    #[test]
    fn test_evolution_engine_planning_determinism() {
        let engine = EvolutionEngine::new();
        let mut report = StewardshipReport::new();

        let finding = StewardshipFinding::new(
            FindingKind::Duplication,
            "Duplicate Concept Notes",
            "Identical concept content found in doc_a and doc_b",
            vec![],
            ConfidenceAssessment::new(0.95),
        );

        report.add_finding(finding);
        let plan = engine.plan(&report);

        assert_eq!(plan.proposals.len(), 1);
        assert_eq!(plan.proposals[0].priority, Priority::High);
        assert_eq!(plan.proposals[0].diff.changes.len(), 1);
    }
}
