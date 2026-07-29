//! Planning runtime orchestration façade coordinating goal decomposition, optimization, compilation, execution planning, and task execution.

use crate::planning::compiler::TaskPlanCompiler;
use crate::planning::decomposer::GoalDecomposer;
use crate::planning::execution_plan::{ExecutionPlan, ExecutionPlanningPolicy};
use crate::planning::execution_planner::ExecutionPlanner;
use crate::planning::execution_runtime::{ExecutionFailure, ExecutionReport, TaskExecutionRuntime};
use crate::planning::models::{GoalIntent, PlanningValidationReport, TaskPlan};
use crate::planning::optimizer::{OptimizationPolicy, OptimizationReport, PlanOptimizer};
use crate::planning::validator::GoalValidator;
use crate::query::context::QueryContextProvider;

/// Orchestration façade for task planning.
pub struct PlanningRuntime {
    decomposer: GoalDecomposer,
    optimizer: PlanOptimizer,
    compiler: TaskPlanCompiler,
    validator: GoalValidator,
    execution_planner: ExecutionPlanner,
    task_execution_runtime: TaskExecutionRuntime,
}

impl Default for PlanningRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanningRuntime {
    /// Instantiates a new `PlanningRuntime`.
    pub fn new() -> Self {
        Self {
            decomposer: GoalDecomposer::new(),
            optimizer: PlanOptimizer::new(),
            compiler: TaskPlanCompiler::new(),
            validator: GoalValidator::new(),
            execution_planner: ExecutionPlanner::new(),
            task_execution_runtime: TaskExecutionRuntime::default(),
        }
    }

    /// Decomposes, optimizes, compiles, and validates a `GoalIntent` into an immutable `TaskPlan` artifact using default policy.
    pub fn create_plan(
        &self,
        goal: &GoalIntent,
        ctx: &dyn QueryContextProvider,
    ) -> Result<(TaskPlan, PlanningValidationReport), String> {
        self.create_plan_with_policy(goal, ctx, &OptimizationPolicy::default())
            .map(|(plan, report, _opt_report)| (plan, report))
    }

    /// Decomposes, optimizes, compiles, and validates a `GoalIntent` into an immutable `TaskPlan` artifact using a custom policy.
    pub fn create_plan_with_policy(
        &self,
        goal: &GoalIntent,
        ctx: &dyn QueryContextProvider,
        policy: &OptimizationPolicy,
    ) -> Result<(TaskPlan, PlanningValidationReport, OptimizationReport), String> {
        let raw_ir = self.decomposer.decompose(goal, ctx);
        let opt_report = self.optimizer.optimize(raw_ir, policy);
        let plan = self.compiler.compile(&opt_report.transformed_ir);
        let val_report = self.validator.validate(&plan);

        if !val_report.is_valid {
            let err_details = val_report
                .errors
                .iter()
                .map(|e| e.details.clone())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!("Planning validation failed: {}", err_details));
        }

        Ok((plan, val_report, opt_report))
    }

    /// Compiles a `GoalIntent` into both an immutable `TaskPlan` and parallel stage-ordered `ExecutionPlan`.
    pub fn create_execution_plan(
        &self,
        goal: &GoalIntent,
        ctx: &dyn QueryContextProvider,
    ) -> Result<(TaskPlan, ExecutionPlan), String> {
        let (task_plan, _) = self.create_plan(goal, ctx)?;
        let exec_plan = self
            .execution_planner
            .plan_execution(&task_plan, &ExecutionPlanningPolicy::default())
            .map_err(|e| e.to_string())?;

        Ok((task_plan, exec_plan))
    }

    /// Executes a `GoalIntent` end-to-end through planning, compilation, stage partitioning, and execution.
    pub fn execute_goal(
        &self,
        goal: &GoalIntent,
        ctx: &dyn QueryContextProvider,
    ) -> Result<ExecutionReport, ExecutionFailure> {
        let (_task_plan, exec_plan) =
            self.create_execution_plan(goal, ctx)
                .map_err(|e| ExecutionFailure {
                    kind: crate::planning::execution_runtime::ExecutionFailureKind::InternalError,
                    task_id: None,
                    message: e,
                })?;

        self.task_execution_runtime.execute_plan(&exec_plan)
    }
}
