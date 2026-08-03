//! StepExecutor trait abstraction, execution context, and type-safe registry.

use brain_domain::{
    DomainError, ExecutionId, ReasoningPlanStep, ReasoningPlanStepKind, StepInputs, StepOutput,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Runtime context passed into step executors carrying cancellation signals, tracing, and telemetry.
#[derive(Debug, Clone)]
pub struct StepExecutionContext {
    /// Strongly-typed execution run ID.
    pub execution_id: ExecutionId,
    /// Cooperative cancellation token.
    pub cancellation_token: CancellationToken,
    /// Optional distributed tracing identifier.
    pub trace_id: Option<String>,
}

impl StepExecutionContext {
    /// Instantiates a new `StepExecutionContext`.
    pub fn new(execution_id: ExecutionId, cancellation_token: CancellationToken) -> Self {
        Self {
            execution_id,
            cancellation_token,
            trace_id: None,
        }
    }
}

/// Internal discriminator enum for mapping plan step kinds to registry entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReasoningPlanStepKindDiscriminator {
    /// Search engine step.
    Search,
    /// Memory query step.
    QueryMemory,
    /// Shared inspection step.
    InspectEntity,
    /// Relationship traversal step.
    TraverseRelationships,
    /// Evidence collection step.
    CollectEvidence,
    /// Response synthesis step.
    SynthesizeResponse,
}

impl From<&ReasoningPlanStepKind> for ReasoningPlanStepKindDiscriminator {
    fn from(kind: &ReasoningPlanStepKind) -> Self {
        match kind {
            ReasoningPlanStepKind::Search { .. } => Self::Search,
            ReasoningPlanStepKind::QueryMemory { .. } => Self::QueryMemory,
            ReasoningPlanStepKind::InspectEntity { .. } => Self::InspectEntity,
            ReasoningPlanStepKind::TraverseRelationships { .. } => Self::TraverseRelationships,
            ReasoningPlanStepKind::CollectEvidence { .. } => Self::CollectEvidence,
            ReasoningPlanStepKind::SynthesizeResponse => Self::SynthesizeResponse,
        }
    }
}

/// Abstract contract for executing an individual reasoning plan step capability.
///
/// Invariant: A `StepExecutor` must be safe to execute at most once for a claimed step.
/// The runtime guarantees a step is scheduled only once per execution.
#[async_trait::async_trait]
pub trait StepExecutor: Send + Sync {
    /// Executes the capability step logic given input context from prerequisite steps.
    async fn execute(
        &self,
        step: &ReasoningPlanStep,
        ctx: &StepExecutionContext,
        inputs: &StepInputs,
    ) -> Result<StepOutput, DomainError>;
}

/// Thread-safe registry mapping step kinds to registered capability executors.
#[derive(Default, Clone)]
pub struct StepExecutorRegistry {
    executors: HashMap<ReasoningPlanStepKindDiscriminator, Arc<dyn StepExecutor>>,
}

impl StepExecutorRegistry {
    /// Creates a new, empty `StepExecutorRegistry`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a capability `StepExecutor` for a given step kind variant.
    pub fn register(
        &mut self,
        step_kind: &ReasoningPlanStepKind,
        executor: Arc<dyn StepExecutor>,
    ) {
        let disc = ReasoningPlanStepKindDiscriminator::from(step_kind);
        self.executors.insert(disc, executor);
    }

    /// Resolves a registered `StepExecutor` for a target step kind.
    pub fn executor_for(
        &self,
        step_kind: &ReasoningPlanStepKind,
    ) -> Result<Arc<dyn StepExecutor>, DomainError> {
        let disc = ReasoningPlanStepKindDiscriminator::from(step_kind);
        self.executors.get(&disc).cloned().ok_or_else(|| {
            DomainError::ValidationError {
                message: format!("No StepExecutor registered for plan step kind {:?}", step_kind),
                rule_id: Some("VAL-EXEC-004".to_string()),
            }
        })
    }
}
