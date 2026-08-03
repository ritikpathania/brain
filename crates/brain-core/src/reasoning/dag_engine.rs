//! ExecutionRunner public orchestration boundary for executing DAG-validated ExecutionPlans.

use crate::reasoning::executor_trait::{StepExecutionContext, StepExecutorRegistry};
use brain_domain::{
    DomainError, ExecutionEvent, ExecutionPlan, ExecutionState, ExecutionTimestamp,
    SkippedReason, StepInputs, StepStatus,
};
use tokio::sync::mpsc::UnboundedSender;

/// Sole public orchestration boundary executing DAG plans, dispatching to StepExecutors, and emitting events.
///
/// Invariants:
/// - Execution is deterministic with respect to the execution plan.
/// - A step becomes runnable exactly once (single runnable dispatch invariant).
/// - Downstream steps depending on a failed step automatically transition to `Skipped`.
#[derive(Clone)]
pub struct ExecutionRunner {
    registry: StepExecutorRegistry,
}

impl ExecutionRunner {
    /// Instantiates a new `ExecutionRunner` with the given executor registry.
    pub fn new(registry: StepExecutorRegistry) -> Self {
        Self { registry }
    }

    /// Executes an `ExecutionPlan` to completion, emitting timestamped progress events on `event_tx`.
    pub async fn run_plan(
        &self,
        plan: &ExecutionPlan,
        ctx: StepExecutionContext,
        event_tx: UnboundedSender<ExecutionEvent>,
    ) -> Result<ExecutionState, DomainError> {
        let mut state = ExecutionState::new();

        // 1. Initial DAG validation check
        plan.validate()?;

        // Main scheduler loop
        loop {
            // Check cooperative cancellation
            if ctx.cancellation_token.is_cancelled() {
                self.skip_remaining_on_cancellation(plan, &mut state, &ctx, &event_tx)?;
                let _ = event_tx.send(ExecutionEvent::PlanFailed {
                    execution_id: ctx.execution_id,
                    error: DomainError::ValidationError {
                        message: "Execution run cancelled by user".to_string(),
                        rule_id: Some("VAL-EXEC-005".to_string()),
                    },
                    occurred_at: ExecutionTimestamp::now(),
                });
                return Ok(state);
            }

            if state.cursor.is_finished(plan) {
                break;
            }

            // Find all candidate ready steps from cursor
            let candidate_steps = state.cursor.next_executable_steps(plan);
            if candidate_steps.is_empty() && !state.cursor.in_flight.is_empty() {
                // Yield thread brief pause to allow active workers to complete
                tokio::task::yield_now().await;
                continue;
            }

            let mut ready_steps = Vec::new();

            for step in candidate_steps {
                // Check if any prerequisite dependency failed or was skipped
                let has_upstream_failure = step.depends_on.iter().any(|dep_id| {
                    matches!(
                        state.status(*dep_id),
                        StepStatus::Failed | StepStatus::Skipped(_)
                    )
                });

                if has_upstream_failure {
                    // Mark step as skipped due to upstream failure
                    state.transition(
                        step.id,
                        StepStatus::Skipped(SkippedReason::UpstreamFailure),
                    )?;
                    let _ = event_tx.send(ExecutionEvent::StepSkipped {
                        execution_id: ctx.execution_id,
                        step_id: step.id,
                        reason: SkippedReason::UpstreamFailure,
                        occurred_at: ExecutionTimestamp::now(),
                    });
                } else {
                    ready_steps.push(step);
                }
            }

            if ready_steps.is_empty() && state.cursor.is_finished(plan) {
                break;
            }

            // Dispatch ready steps concurrently
            let mut futures = Vec::new();

            for step in ready_steps {
                // Single runnable dispatch invariant: atomically transition step to Running
                state.transition(step.id, StepStatus::Running)?;

                let _ = event_tx.send(ExecutionEvent::StepStarted {
                    execution_id: ctx.execution_id,
                    step_id: step.id,
                    occurred_at: ExecutionTimestamp::now(),
                });

                // Gather completed prerequisite artifact IDs
                let mut prerequisite_ids = Vec::new();
                for dep_id in &step.depends_on {
                    if let Some(art) = state.artifact_store.get_by_producer(*dep_id) {
                        prerequisite_ids.push(art.id());
                    }
                }
                let inputs = StepInputs { prerequisite_ids };

                let executor = self.registry.executor_for(&step.kind)?;
                let step_clone = step.clone();
                let ctx_clone = ctx.clone();

                futures.push(async move {
                    let result = executor.execute(&step_clone, &ctx_clone, &inputs).await;
                    (step_clone, result)
                });
            }

            if futures.is_empty() && !state.cursor.is_finished(plan) {
                // All remaining steps skipped
                continue;
            }

            // Await ready step worker futures concurrently
            let results = futures::future::join_all(futures).await;

            for (step, res) in results {
                let step_id = step.id;
                match res {
                    Ok(output) => {
                        state.transition(step_id, StepStatus::Completed)?;

                        // Construct ExecutionArtifact via ArtifactBuilder
                        let artifact = crate::reasoning::ArtifactBuilder::build(&step, &ctx, output.clone());
                        let artifact_id = artifact.id;

                        // Insert artifact and record provenance edges into state.artifact_store
                        state.artifact_store.insert(artifact)?;

                        for dep_id in &step.depends_on {
                            if let Some(parent_art) = state.artifact_store.get_by_producer(*dep_id) {
                                let edge = brain_domain::ProvenanceEdge::new(
                                    parent_art.id(),
                                    artifact_id,
                                    brain_domain::ProvenanceRelationship::DerivedFrom,
                                    ExecutionTimestamp::now(),
                                );
                                let _ = state.artifact_store.add_edge(edge);
                            }
                        }

                        state.set_output(step_id, output)?;
                        let _ = event_tx.send(ExecutionEvent::StepCompleted {
                            execution_id: ctx.execution_id,
                            step_id,
                            occurred_at: ExecutionTimestamp::now(),
                        });
                    }
                    Err(err) => {
                        state.transition(step_id, StepStatus::Failed)?;
                        state.set_error(step_id, err.clone())?;
                        let _ = event_tx.send(ExecutionEvent::StepFailed {
                            execution_id: ctx.execution_id,
                            step_id,
                            error: err,
                            occurred_at: ExecutionTimestamp::now(),
                        });
                    }
                }
            }
        }

        // Final event notification
        let has_failures = !state.failed_steps().is_empty();
        if has_failures {
            let _ = event_tx.send(ExecutionEvent::PlanFailed {
                execution_id: ctx.execution_id,
                error: DomainError::ValidationError {
                    message: "Plan execution completed with step failures".to_string(),
                    rule_id: Some("VAL-EXEC-006".to_string()),
                },
                occurred_at: ExecutionTimestamp::now(),
            });
        } else {
            let _ = event_tx.send(ExecutionEvent::PlanCompleted {
                execution_id: ctx.execution_id,
                occurred_at: ExecutionTimestamp::now(),
            });
        }

        Ok(state)
    }

    fn skip_remaining_on_cancellation(
        &self,
        plan: &ExecutionPlan,
        state: &mut ExecutionState,
        ctx: &StepExecutionContext,
        event_tx: &UnboundedSender<ExecutionEvent>,
    ) -> Result<(), DomainError> {
        for step in &plan.steps {
            if matches!(
                state.status(step.id),
                StepStatus::Pending | StepStatus::Ready
            ) {
                state.transition(step.id, StepStatus::Skipped(SkippedReason::Cancelled))?;
                let _ = event_tx.send(ExecutionEvent::StepSkipped {
                    execution_id: ctx.execution_id,
                    step_id: step.id,
                    reason: SkippedReason::Cancelled,
                    occurred_at: ExecutionTimestamp::now(),
                });
            }
        }
        Ok(())
    }
}
