/// Domain models for Knowledge Evolution Planning, Validation, and Execution (Phase 6 Milestone 6.2).
pub mod models_v2;
pub use models_v2::*;

/// Source-agnostic evolution planner composing immutable KnowledgeEvolutionPlan artifacts.
pub mod planner_v2;
pub use planner_v2::EvolutionPlannerV2;

/// Pure validator executing structural and safety invariant checks over KnowledgeEvolutionPlan.
pub mod validator_v2;
pub use validator_v2::PlanValidatorV2;

/// Transactional execution engine translating validated KnowledgeEvolutionPlan items into intent mutation sets.
pub mod executor_v2;
pub use executor_v2::EvolutionExecutorV2;

/// Governance evolution policies and deterministic manager.
pub mod policy;

/// Knowledge evolution planner constructing, simulating, and executing plans.
pub mod planner;

pub use planner::KnowledgeEvolutionPlanner;
pub use policy::EvolutionPolicyManager;
