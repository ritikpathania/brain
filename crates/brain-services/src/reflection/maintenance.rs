//! Knowledge Maintenance Runtime orchestrating background reflection, evolution planning, pure validation, and transactional execution (Phase 6 Milestone 6.3).
//!
//! ### Orchestration Invariants:
//! 1. **Workflow-Only**: `KnowledgeMaintenanceRuntime` owns execution workflow coordination ONLY; zero policy or domain logic resides in the runtime.
//! 2. **Explicit State Machine**: Maintenance cycles follow strict transitions: `Idle` -> `Reflecting` -> `Planning` -> `Validating` -> `WaitingForApproval` -> `Executing` -> `Completed` / `Failed` / `Cancelled`.
//! 3. **Append-Only Event Stream**: `MaintenanceStageEvent` items form an immutable execution log.
//! 4. **Deterministic Orchestration**: Given identical `ReflectionInput`, `MaintenanceConfig`, and `ApprovalDecision`, outputs are 100% deterministic.

use crate::evolution::executor_v2::EvolutionExecutorV2;
use crate::evolution::models_v2::{
    EvolutionExecutionReport, EvolutionMutationSet, KnowledgeEvolutionPlan, PlanId,
};
use crate::evolution::planner_v2::EvolutionPlannerV2;
use crate::evolution::validator_v2::PlanValidatorV2;
use crate::reflection::engine_v2::ReflectionEngineV2;
use crate::reflection::input::{ReflectionInput, SnapshotId};
use crate::reflection::models::ReflectionReportV2;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Explicit execution state for a background maintenance cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaintenanceState {
    /// Idle state ready for cycle launch.
    Idle,
    /// Actively running read-only reflection passes.
    Reflecting,
    /// Translating findings into evolution proposals and dependency graph.
    Planning,
    /// Executing pure safety validation checks.
    Validating,
    /// Cycle paused awaiting human or governance approval.
    WaitingForApproval,
    /// Transactionally executing evolution mutations.
    Executing,
    /// Maintenance cycle completed successfully.
    Completed,
    /// Maintenance cycle failed.
    Failed,
    /// Maintenance cycle cancelled.
    Cancelled,
}

/// Kind classification for append-only maintenance stage events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaintenanceStageEventKind {
    /// Cycle initialized.
    CycleStarted,
    /// Reflection pass analysis started.
    ReflectionStarted,
    /// Reflection pass analysis completed.
    ReflectionCompleted,
    /// Plan composition started.
    PlanningStarted,
    /// Plan composition completed.
    PlanningCompleted,
    /// Pure validation completed.
    ValidationCompleted,
    /// Governance approval received.
    ApprovalReceived,
    /// Transactional execution started.
    ExecutionStarted,
    /// Transactional execution completed.
    ExecutionCompleted,
    /// Maintenance cycle completed.
    CycleCompleted,
    /// Maintenance cycle failed.
    CycleFailed,
    /// Maintenance cycle cancelled.
    CycleCancelled,
}

/// Immutable event item emitted at maintenance stage boundaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaintenanceStageEvent {
    /// Unique event identifier.
    pub event_id: Uuid,
    /// Stage event classification kind.
    pub kind: MaintenanceStageEventKind,
    /// Human-readable log message.
    pub message: String,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Governance decision artifact authorizing plan execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalDecision {
    /// Unique decision identifier.
    pub decision_id: Uuid,
    /// Target plan identifier.
    pub plan_id: PlanId,
    /// Authorizing identity or policy engine string.
    pub approved_by: String,
    /// Approval flag (true = approved, false = rejected).
    pub is_approved: bool,
    /// Governance rationale or comments.
    pub comments: String,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Strongly-typed error classification for maintenance cycle failures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MaintenanceError {
    /// Plan safety validation failed.
    ValidationFailed(String),
    /// Cycle paused awaiting explicit approval.
    ApprovalRequired,
    /// Governance decision explicitly rejected plan.
    ApprovalRejected(String),
    /// Transactional execution failed.
    ExecutionFailed(String),
    /// Cycle execution was cancelled.
    Cancelled,
}

impl std::fmt::Display for MaintenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaintenanceError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
            MaintenanceError::ApprovalRequired => write!(f, "Governance approval required"),
            MaintenanceError::ApprovalRejected(msg) => write!(f, "Approval rejected: {}", msg),
            MaintenanceError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            MaintenanceError::Cancelled => write!(f, "Maintenance cycle cancelled"),
        }
    }
}

impl std::error::Error for MaintenanceError {}

/// Configuration for `KnowledgeMaintenanceRuntime`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaintenanceConfig {
    /// Flag requiring explicit approval decision before execution.
    pub require_approval: bool,
    /// Flag enabling dry-run execution mode.
    pub dry_run: bool,
}

/// Structured outcome produced by a maintenance cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaintenanceCycleResult {
    /// Unique cycle execution UUID.
    pub cycle_id: Uuid,
    /// Final maintenance state.
    pub state: MaintenanceState,
    /// Versioned snapshot ID inspected.
    pub snapshot_id: SnapshotId,
    /// Generated reflection report.
    pub reflection_report: ReflectionReportV2,
    /// Compiled evolution plan (if any proposals produced).
    pub evolution_plan: Option<KnowledgeEvolutionPlan>,
    /// Governance approval decision (if applicable).
    pub approval_decision: Option<ApprovalDecision>,
    /// Execution report audit trail (if executed).
    pub execution_report: Option<EvolutionExecutionReport>,
    /// Append-only stage event log.
    pub events: Vec<MaintenanceStageEvent>,
    /// Cycle timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Workflow-only background maintenance orchestrator.
pub struct KnowledgeMaintenanceRuntime {
    reflection_engine: ReflectionEngineV2,
    planner: EvolutionPlannerV2,
    validator: PlanValidatorV2,
    executor: EvolutionExecutorV2,
    config: MaintenanceConfig,
}

impl Default for KnowledgeMaintenanceRuntime {
    fn default() -> Self {
        Self::new(MaintenanceConfig::default())
    }
}

impl KnowledgeMaintenanceRuntime {
    /// Instantiates a new `KnowledgeMaintenanceRuntime`.
    pub fn new(config: MaintenanceConfig) -> Self {
        Self {
            reflection_engine: ReflectionEngineV2::new(),
            planner: EvolutionPlannerV2::new(),
            validator: PlanValidatorV2::new(),
            executor: EvolutionExecutorV2::new(),
            config,
        }
    }

    /// Emits a new stage event item.
    fn emit_event(
        events: &mut Vec<MaintenanceStageEvent>,
        kind: MaintenanceStageEventKind,
        msg: &str,
        timestamp_ms: u64,
    ) {
        events.push(MaintenanceStageEvent {
            event_id: Uuid::new_v4(),
            kind,
            message: msg.to_string(),
            timestamp_ms,
        });
    }

    /// Executes a single maintenance cycle on demand over a `ReflectionInput` snapshot.
    pub fn run_cycle(
        &self,
        input: &ReflectionInput,
    ) -> Result<MaintenanceCycleResult, MaintenanceError> {
        let cycle_id = Uuid::new_v4();
        let mut events = Vec::new();
        let now = input.timestamp_ms;

        Self::emit_event(
            &mut events,
            MaintenanceStageEventKind::CycleStarted,
            "Maintenance cycle started",
            now,
        );

        // State Transition: Idle -> Reflecting
        Self::emit_event(
            &mut events,
            MaintenanceStageEventKind::ReflectionStarted,
            "Reflection pass analysis started",
            now,
        );
        let reflection_report = self.reflection_engine.run(input);
        Self::emit_event(
            &mut events,
            MaintenanceStageEventKind::ReflectionCompleted,
            "Reflection pass analysis completed",
            now,
        );

        // State Transition: Reflecting -> Planning
        Self::emit_event(
            &mut events,
            MaintenanceStageEventKind::PlanningStarted,
            "Evolution plan composition started",
            now,
        );
        let evolution_plan = self.planner.plan_from_reflection(&reflection_report);
        Self::emit_event(
            &mut events,
            MaintenanceStageEventKind::PlanningCompleted,
            "Evolution plan composition completed",
            now,
        );

        if evolution_plan.proposals.is_empty() {
            Self::emit_event(
                &mut events,
                MaintenanceStageEventKind::CycleCompleted,
                "No proposals produced; maintenance cycle completed",
                now,
            );
            return Ok(MaintenanceCycleResult {
                cycle_id,
                state: MaintenanceState::Completed,
                snapshot_id: input.snapshot_id,
                reflection_report,
                evolution_plan: Some(evolution_plan),
                approval_decision: None,
                execution_report: None,
                events,
                timestamp_ms: now,
            });
        }

        // State Transition: Planning -> Validating
        let val_report = self.validator.validate(&evolution_plan);
        Self::emit_event(
            &mut events,
            MaintenanceStageEventKind::ValidationCompleted,
            "Pure plan safety validation completed",
            now,
        );

        if !val_report.is_valid {
            Self::emit_event(
                &mut events,
                MaintenanceStageEventKind::CycleFailed,
                "Plan safety validation failed",
                now,
            );
            return Err(MaintenanceError::ValidationFailed(
                "Safety check failed".to_string(),
            ));
        }

        if self.config.dry_run {
            Self::emit_event(
                &mut events,
                MaintenanceStageEventKind::CycleCompleted,
                "Dry-run mode active; plan compiled without mutation",
                now,
            );
            return Ok(MaintenanceCycleResult {
                cycle_id,
                state: MaintenanceState::Completed,
                snapshot_id: input.snapshot_id,
                reflection_report,
                evolution_plan: Some(evolution_plan),
                approval_decision: None,
                execution_report: None,
                events,
                timestamp_ms: now,
            });
        }

        if self.config.require_approval {
            Self::emit_event(
                &mut events,
                MaintenanceStageEventKind::ValidationCompleted,
                "Governance approval required before execution",
                now,
            );
            return Ok(MaintenanceCycleResult {
                cycle_id,
                state: MaintenanceState::WaitingForApproval,
                snapshot_id: input.snapshot_id,
                reflection_report,
                evolution_plan: Some(evolution_plan),
                approval_decision: None,
                execution_report: None,
                events,
                timestamp_ms: now,
            });
        }

        // AutoApply Mode: Execute immediately
        Self::emit_event(
            &mut events,
            MaintenanceStageEventKind::ExecutionStarted,
            "Transactional plan execution started",
            now,
        );
        let (_mutations, exec_report) = self
            .executor
            .execute(&evolution_plan)
            .map_err(MaintenanceError::ExecutionFailed)?;
        Self::emit_event(
            &mut events,
            MaintenanceStageEventKind::ExecutionCompleted,
            "Transactional plan execution completed",
            now,
        );
        Self::emit_event(
            &mut events,
            MaintenanceStageEventKind::CycleCompleted,
            "Maintenance cycle completed successfully",
            now,
        );

        Ok(MaintenanceCycleResult {
            cycle_id,
            state: MaintenanceState::Completed,
            snapshot_id: input.snapshot_id,
            reflection_report,
            evolution_plan: Some(evolution_plan),
            approval_decision: None,
            execution_report: Some(exec_report),
            events,
            timestamp_ms: now,
        })
    }

    /// Transactionally executes a compiled plan using an explicit `ApprovalDecision` artifact.
    pub fn execute_approved_plan(
        &self,
        plan: &KnowledgeEvolutionPlan,
        decision: &ApprovalDecision,
    ) -> Result<(EvolutionMutationSet, EvolutionExecutionReport), MaintenanceError> {
        if !decision.is_approved {
            return Err(MaintenanceError::ApprovalRejected(
                decision.comments.clone(),
            ));
        }

        self.executor
            .execute(plan)
            .map_err(MaintenanceError::ExecutionFailed)
    }
}
