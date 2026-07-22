/// Declarative task definitions and priority types.
pub mod task;
pub use task::{
    MaintenanceMode, OrchestratorTask, TaskId, TaskKind, TaskPriority, TaskStatus, TaskTraceRecord,
};

/// Priority queue with dependency resolution and backpressure rules.
pub mod priority_queue;
pub use priority_queue::PriorityTaskQueue;

/// Subsystem executor trait and default dispatcher.
pub mod executor;
pub use executor::{DefaultSubsystemExecutor, TaskExecutor};

/// SQLite WAL checkpointing and database maintenance.
pub mod maintenance;
pub use maintenance::MaintenanceEngine;

/// Deterministic single-loop background orchestrator.
pub mod runtime_orchestrator;
pub use runtime_orchestrator::{OrchestratorDiagnosticsSnapshot, RuntimeOrchestrator};
