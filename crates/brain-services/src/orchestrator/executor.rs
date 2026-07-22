use super::maintenance::MaintenanceEngine;
use super::task::{MaintenanceMode, OrchestratorTask, TaskKind};
use crate::projections::{ProjectionId, ProjectionScheduler};
use crate::reflection::BackgroundReflectionScheduler;
use brain_core::errors::BrainError;
use std::sync::Arc;

/// Trait defining declarative task execution mechanics for the orchestrator.
pub trait TaskExecutor: Send + Sync {
    /// Dispatches a declarative task to its target subsystem handler.
    fn execute(&self, task: &OrchestratorTask) -> Result<(), BrainError>;
}

/// Unified subsystem executor delegating task kinds to underlying engine components.
pub struct DefaultSubsystemExecutor {
    projection_scheduler: Arc<dyn ProjectionScheduler>,
    reflection_scheduler: Arc<BackgroundReflectionScheduler>,
    maintenance_engine: Arc<MaintenanceEngine>,
}

impl DefaultSubsystemExecutor {
    /// Creates a new `DefaultSubsystemExecutor`.
    pub fn new(
        projection_scheduler: Arc<dyn ProjectionScheduler>,
        reflection_scheduler: Arc<BackgroundReflectionScheduler>,
        maintenance_engine: Arc<MaintenanceEngine>,
    ) -> Self {
        Self {
            projection_scheduler,
            reflection_scheduler,
            maintenance_engine,
        }
    }
}

impl TaskExecutor for DefaultSubsystemExecutor {
    fn execute(&self, task: &OrchestratorTask) -> Result<(), BrainError> {
        match &task.kind {
            TaskKind::Compile => {
                // Ingestion canonicalization completes inline during ingest API calls;
                // compile task serves as a sequence synchronization checkpoint.
                Ok(())
            }
            TaskKind::Project { name } => match name {
                Some(proj_name) => {
                    if let Some(id) = parse_projection_id(proj_name) {
                        self.projection_scheduler.catch_up_projection(id)
                    } else {
                        self.projection_scheduler.catch_up_all()
                    }
                }
                None => self.projection_scheduler.catch_up_all(),
            },
            TaskKind::Reflect { force } => self.reflection_scheduler.run_cycle(*force),
            TaskKind::Maintain { mode } => match mode {
                MaintenanceMode::PeriodicWalCheckpoint => self.maintenance_engine.checkpoint_wal(),
                MaintenanceMode::OpportunisticVacuum => self.maintenance_engine.vacuum(),
            },
        }
    }
}

fn parse_projection_id(name: &str) -> Option<ProjectionId> {
    match name.to_lowercase().as_str() {
        "jobs" => Some(ProjectionId::Jobs),
        "sessions" => Some(ProjectionId::Sessions),
        "search" => Some(ProjectionId::Search),
        "retrieval" => Some(ProjectionId::Retrieval),
        "test_a" => Some(ProjectionId::TestA),
        "test_b" => Some(ProjectionId::TestB),
        "test_c" => Some(ProjectionId::TestC),
        _ => None,
    }
}
