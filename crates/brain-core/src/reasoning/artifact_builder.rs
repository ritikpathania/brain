//! Centralized builder for constructing ExecutionArtifacts from step execution outputs and plan metadata.

use crate::reasoning::executor_trait::StepExecutionContext;
use brain_domain::{
    ArtifactMetadata, EvidenceArtifactKind, ExecutionArtifact, ReasoningPlanStep,
    ReasoningPlanStepKind, StepOutput,
};

/// Builder utility responsible for constructing `ExecutionArtifact` value objects consistently.
pub struct ArtifactBuilder;

impl ArtifactBuilder {
    /// Builds an `ExecutionArtifact` from a step output payload, target plan step, and execution context.
    pub fn build(
        step: &ReasoningPlanStep,
        ctx: &StepExecutionContext,
        output: StepOutput,
    ) -> ExecutionArtifact {
        let kind = Self::infer_kind(&step.kind);

        let metadata = ArtifactMetadata {
            kind,
            producer_step: step.id,
            execution_id: ctx.execution_id,
            created_at: brain_domain::ExecutionTimestamp::now(),
        };

        ExecutionArtifact::new(metadata, output.value)
    }

    /// Infers representation `EvidenceArtifactKind` from capability step kind.
    fn infer_kind(step_kind: &ReasoningPlanStepKind) -> EvidenceArtifactKind {
        match step_kind {
            ReasoningPlanStepKind::Search { .. } | ReasoningPlanStepKind::QueryMemory { .. } => {
                EvidenceArtifactKind::RawData
            }
            ReasoningPlanStepKind::InspectEntity { .. }
            | ReasoningPlanStepKind::TraverseRelationships { .. } => {
                EvidenceArtifactKind::DerivedData
            }
            ReasoningPlanStepKind::CollectEvidence { .. } => EvidenceArtifactKind::Summary,
            ReasoningPlanStepKind::SynthesizeResponse => EvidenceArtifactKind::Result,
        }
    }
}
