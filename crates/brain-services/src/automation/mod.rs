/// Automation orchestration rules and deterministic manager.
pub mod rule;

/// Background Automation Scheduler with queue state machine and execution traceability.
pub mod scheduler;

pub use rule::{AutomationRuleManager, RuntimeSnapshot};
pub use scheduler::AutomationScheduler;
