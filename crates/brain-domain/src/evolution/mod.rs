//! Knowledge Evolution domain model.
//!
//! This module defines presentation-agnostic domain models for planning,
//! reviewing, and executing structured graph evolutions with semantic diffs and rollback.

/// Individual graph mutation actions.
pub mod action;
/// Semantic graph diffs.
pub mod diff;
/// Evolution engine pure planner.
pub mod engine;
/// Transactional execution records.
pub mod execution;
/// Evolution plan aggregate.
pub mod plan;
/// Evolution proposal aggregate.
pub mod proposal;

/// Legacy evolution models.
pub mod legacy;

pub use action::{ActionId, EvolutionAction, EvolutionActionKind};
pub use diff::{SemanticChange, SemanticDiff};
pub use engine::EvolutionEngine;
pub use execution::{EvolutionExecution, EvolutionExecutionId, ExecutionResult};
pub use legacy::{
    DomainEntityId, EvolutionActionId, EvolutionPlanId, LegacyEvolutionAction, LegacyEvolutionPlan,
};
pub use plan::EvolutionPlan;
pub use proposal::{EvolutionProposal, Priority, ProposalId, ProposalOrigin, ProposalStatus};
