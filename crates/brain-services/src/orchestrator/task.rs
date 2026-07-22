use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use uuid::Uuid;

/// Unique identifier for an orchestrator task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Creates a new random `TaskId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Task execution priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TaskPriority {
    /// Maintenance tasks (e.g. WAL checkpoints, opportunistic vacuuming).
    Low = 0,
    /// Background self-optimization passes (reflection engine).
    Normal = 1,
    /// Read-model projection catch-up ticks.
    High = 2,
    /// Ingestion canonicalization & Knowledge Compiler operations.
    Critical = 3,
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Operational maintenance mode targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaintenanceMode {
    /// Passive WAL checkpointing (`PRAGMA wal_checkpoint(PASSIVE)`).
    PeriodicWalCheckpoint,
    /// Database vacuum and index optimization during idle windows.
    OpportunisticVacuum,
}

/// Declarative task category variants dispatched to dedicated subsystem handlers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskKind {
    /// Knowledge compiler canonicalization step.
    Compile,
    /// Projection read-model catch-up tick.
    Project {
        /// Optional projection name filter.
        name: Option<String>,
    },
    /// Reflection self-optimization pass.
    Reflect {
        /// Forces run regardless of event sequence thresholds.
        force: bool,
    },
    /// Database maintenance operation.
    Maintain {
        /// Mode of maintenance to execute.
        mode: MaintenanceMode,
    },
}

impl fmt::Display for TaskKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile => write!(f, "compile"),
            Self::Project { name } => match name {
                Some(n) => write!(f, "project({})", n),
                None => write!(f, "project(all)"),
            },
            Self::Reflect { force } => write!(f, "reflect(force={})", force),
            Self::Maintain { mode } => match mode {
                MaintenanceMode::PeriodicWalCheckpoint => write!(f, "maintain(wal_checkpoint)"),
                MaintenanceMode::OpportunisticVacuum => write!(f, "maintain(vacuum)"),
            },
        }
    }
}

/// Declarative task description scheduled by the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrchestratorTask {
    /// Unique task ID.
    pub id: TaskId,
    /// Declarative category of the task.
    pub kind: TaskKind,
    /// Scheduling priority level.
    pub priority: TaskPriority,
    /// List of parent `TaskId`s that must complete before this task can run.
    pub dependencies: Vec<TaskId>,
    /// Unix timestamp in milliseconds when the task was created.
    pub created_at_unix_ms: u64,
    /// Optional execution timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Metadata tags attached to the task for auditing and tracing.
    pub metadata: BTreeMap<String, String>,
}

impl OrchestratorTask {
    /// Creates a new declarative `OrchestratorTask`.
    pub fn new(kind: TaskKind, priority: TaskPriority) -> Self {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            id: TaskId::new(),
            kind,
            priority,
            dependencies: Vec::new(),
            created_at_unix_ms: now_ms,
            timeout_ms: None,
            metadata: BTreeMap::new(),
        }
    }

    /// Builder method to add a parent task dependency.
    pub fn with_dependency(mut self, parent_id: TaskId) -> Self {
        self.dependencies.push(parent_id);
        self
    }

    /// Builder method to set execution timeout in milliseconds.
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }
}
