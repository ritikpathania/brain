//! Source-agnostic `EvolutionPlannerV2` translating reflection findings into compiled `KnowledgeEvolutionPlan` artifacts (Phase 6 Milestone 6.2).

use crate::evolution::models_v2::{
    EvolutionActionKind, KnowledgeEvolutionPlan, KnowledgeEvolutionProposal, PlanId,
    ProposalDependencyEdge, ProposalGraph, ProposalId,
};
use crate::reflection::models::{ReflectionFindingKind, ReflectionReportV2};
use uuid::Uuid;

/// Source-agnostic evolution planner composing immutable `KnowledgeEvolutionPlan` artifacts.
#[derive(Debug, Clone, Default)]
pub struct EvolutionPlannerV2;

impl EvolutionPlannerV2 {
    /// Instantiates a new `EvolutionPlannerV2`.
    pub fn new() -> Self {
        Self
    }

    /// Translates a domain `ReflectionReportV2` into a compiled `KnowledgeEvolutionPlan`.
    pub fn plan_from_reflection(&self, report: &ReflectionReportV2) -> KnowledgeEvolutionPlan {
        let mut proposals = Vec::new();
        let mut graph = ProposalGraph::new();

        for finding in &report.findings {
            match &finding.kind {
                ReflectionFindingKind::DuplicateEntity(details) => {
                    if details.entity_ids.len() >= 2 {
                        let prop_id = ProposalId(Uuid::new_v4());
                        let prop = KnowledgeEvolutionProposal {
                            id: prop_id,
                            action: EvolutionActionKind::MergeEntities {
                                target_id: details.entity_ids[0].clone(),
                                source_id: details.entity_ids[1].clone(),
                            },
                            reasoning: format!(
                                "Duplicate entity candidate merge with similarity {:.2}",
                                details.similarity_score
                            ),
                            confidence: finding.confidence,
                        };
                        proposals.push(prop.clone());
                        graph.nodes.push(prop);
                    }
                }
                ReflectionFindingKind::AttributeContradiction(details) => {
                    for fact_id in &details.conflicting_fact_ids {
                        let prop_id = ProposalId(Uuid::new_v4());
                        let prop = KnowledgeEvolutionProposal {
                            id: prop_id,
                            action: EvolutionActionKind::SupercedeFact {
                                target_entity_id: details.entity_id.clone(),
                                stale_fact_id: fact_id.clone(),
                            },
                            reasoning: details.description.clone(),
                            confidence: finding.confidence,
                        };
                        proposals.push(prop.clone());
                        graph.nodes.push(prop);
                    }
                }
                ReflectionFindingKind::ConfidenceDecay(details) => {
                    let prop_id = ProposalId(Uuid::new_v4());
                    let prop = KnowledgeEvolutionProposal {
                        id: prop_id,
                        action: EvolutionActionKind::UpdateConfidence {
                            target_entity_id: details.entity_id.clone(),
                            new_confidence: details.new_confidence,
                        },
                        reasoning: format!(
                            "Confidence decay adjustment from {:.2} to {:.2}",
                            details.old_confidence, details.new_confidence
                        ),
                        confidence: finding.confidence,
                    };
                    proposals.push(prop.clone());
                    graph.nodes.push(prop);
                }
                ReflectionFindingKind::OrphanEntity(_) => {}
            }
        }

        // Infer dependency edges: Merge proposals precede confidence updates for the same target
        for i in 0..proposals.len() {
            for j in 0..proposals.len() {
                if i != j {
                    if let (
                        EvolutionActionKind::SupercedeFact {
                            target_entity_id: id1,
                            ..
                        },
                        EvolutionActionKind::MergeEntities { target_id: id2, .. },
                    ) = (&proposals[i].action, &proposals[j].action)
                    {
                        if id1 == id2 {
                            graph.edges.push(ProposalDependencyEdge {
                                source: proposals[i].id,
                                target: proposals[j].id,
                            });
                        }
                    }
                }
            }
        }

        KnowledgeEvolutionPlan {
            plan_id: PlanId(Uuid::new_v4()),
            proposals,
            dependency_graph: graph,
            timestamp_ms: report.timestamp_ms,
        }
    }
}
