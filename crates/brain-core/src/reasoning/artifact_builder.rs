//! Centralized builder for constructing ExecutionArtifacts from step execution outputs and plan metadata.

use crate::reasoning::executor_trait::StepExecutionContext;
use brain_domain::{
    ArtifactMetadata, EvidenceArtifactKind, ExecutionArtifact, ReasoningPlanStep,
    ReasoningPlanStepKind, StepOutput,
};

/// Builder utility responsible for constructing `ExecutionArtifact` value objects consistently.
pub struct ArtifactBuilder;

impl ArtifactBuilder {
    /// Builds an `ExecutionArtifact` from an `ArtifactDescriptor`, target plan step, and execution context.
    pub fn build_from_descriptor(
        step: &ReasoningPlanStep,
        ctx: &StepExecutionContext,
        descriptor: brain_domain::ArtifactDescriptor,
    ) -> ExecutionArtifact {
        let metadata = ArtifactMetadata {
            kind: descriptor.kind,
            producer_step: step.id,
            execution_id: ctx.execution_id,
            created_at: brain_domain::ExecutionTimestamp::now(),
        };

        ExecutionArtifact::new(metadata, descriptor.value)
    }

    /// Builds an `ExecutionArtifact` from a step output payload, inferring representation kind from step kind.
    pub fn build(
        step: &ReasoningPlanStep,
        ctx: &StepExecutionContext,
        output: StepOutput,
    ) -> ExecutionArtifact {
        let kind = Self::infer_kind(&step.kind);
        let descriptor = brain_domain::ArtifactDescriptor::new(kind, output.value);
        Self::build_from_descriptor(step, ctx, descriptor)
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
