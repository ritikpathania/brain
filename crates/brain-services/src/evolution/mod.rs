/// Governance evolution policies and deterministic manager.
pub mod policy;

/// Knowledge evolution planner constructing, simulating, and executing plans.
pub mod planner;

pub use planner::KnowledgeEvolutionPlanner;
pub use policy::EvolutionPolicyManager;
