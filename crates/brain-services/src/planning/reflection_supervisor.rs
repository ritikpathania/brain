//! Supervision engine, lightweight stage checkpointing, and resumable recovery runtime.

use crate::planning::retry_policy::RetryPolicy;
use crate::reflection::context::TaskReflectionContext;
use crate::reflection::dag::TaskDag;
use crate::reflection::result::ReflectionResult;
use brain_domain::{CanonicalEntity, EntityId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

/// Supported checkpoint format schema version.
pub const CURRENT_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

/// Lightweight execution checkpoint artifact capturing stage progress and modified entity IDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Schema format version for compatibility validation.
    pub schema_version: u32,
    /// Execution plan identifier.
    pub plan_id: String,
    /// Index of the last successfully completed DAG stage.
    pub completed_stage_index: usize,
    /// Set of task IDs completed up to this stage.
    pub completed_task_ids: Vec<String>,
    /// Accumulated modified entity IDs.
    pub modified_entity_ids: Vec<EntityId>,
    /// Timestamp when checkpoint was saved.
    pub created_at_ms: u64,
}

impl Checkpoint {
    /// Creates a new `Checkpoint` at `CURRENT_CHECKPOINT_SCHEMA_VERSION`.
    pub fn new(
        plan_id: impl Into<String>,
        stage_index: usize,
        completed_task_ids: Vec<String>,
        modified_entity_ids: Vec<EntityId>,
        created_at_ms: u64,
    ) -> Self {
        Self {
            schema_version: CURRENT_CHECKPOINT_SCHEMA_VERSION,
            plan_id: plan_id.into(),
            completed_stage_index: stage_index,
            completed_task_ids,
            modified_entity_ids,
            created_at_ms,
        }
    }
}

/// Storage runtime managing in-memory or durable checkpoint persistence.
#[derive(Default)]
pub struct CheckpointRuntime {
    store: HashMap<String, Checkpoint>,
}

impl CheckpointRuntime {
    /// Creates a new `CheckpointRuntime`.
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
        }
    }

    /// Persists a lightweight stage checkpoint.
    pub fn save_checkpoint(&mut self, checkpoint: Checkpoint) {
        self.store.insert(checkpoint.plan_id.clone(), checkpoint);
    }

    /// Loads a checkpoint for a plan ID, returning an error if schema_version is unsupported.
    pub fn load_checkpoint(&self, plan_id: &str) -> Result<Option<Checkpoint>, String> {
        if let Some(cp) = self.store.get(plan_id) {
            if cp.schema_version > CURRENT_CHECKPOINT_SCHEMA_VERSION {
                return Err(format!(
                    "Unsupported checkpoint schema version {} (current max supported is {})",
                    cp.schema_version, CURRENT_CHECKPOINT_SCHEMA_VERSION
                ));
            }
            Ok(Some(cp.clone()))
        } else {
            Ok(None)
        }
    }
}

/// Recovery runtime evaluating checkpoints to resume execution after interruption.
pub struct RecoveryRuntime;

impl RecoveryRuntime {
    /// Replays a checkpoint to determine the stage index from which execution should resume.
    pub fn prepare_resumption(checkpoint: &Checkpoint) -> Result<usize, String> {
        if checkpoint.schema_version > CURRENT_CHECKPOINT_SCHEMA_VERSION {
            return Err(format!(
                "Cannot resume: checkpoint schema version {} is incompatible",
                checkpoint.schema_version
            ));
        }
        // Resume from the next stage following the last completed stage
        Ok(checkpoint.completed_stage_index + 1)
    }
}

use brain_events::{ReflectionEventBus, ReflectionRuntimeEvent};
use std::sync::Arc;

/// Supervisor orchestrating DAG execution, checking state checkpoints, and enforcing retries.
pub struct ReflectionSupervisor {
    checkpoint_runtime: CheckpointRuntime,
    retry_policy: RetryPolicy,
    event_bus: Option<Arc<ReflectionEventBus>>,
}

impl Default for ReflectionSupervisor {
    fn default() -> Self {
        Self {
            checkpoint_runtime: CheckpointRuntime::new(),
            retry_policy: RetryPolicy::default(),
            event_bus: None,
        }
    }
}

impl ReflectionSupervisor {
    /// Creates a new `ReflectionSupervisor`.
    pub fn new(checkpoint_runtime: CheckpointRuntime, retry_policy: RetryPolicy) -> Self {
        Self {
            checkpoint_runtime,
            retry_policy,
            event_bus: None,
        }
    }

    /// Attaches an event bus for runtime progress event streaming.
    pub fn with_event_bus(mut self, event_bus: Arc<ReflectionEventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Returns a reference to the internal checkpoint runtime.
    pub fn checkpoint_runtime(&self) -> &CheckpointRuntime {
        &self.checkpoint_runtime
    }

    /// Returns a mutable reference to the internal checkpoint runtime.
    pub fn checkpoint_runtime_mut(&mut self) -> &mut CheckpointRuntime {
        &mut self.checkpoint_runtime
    }

    /// Supervise execution of a `TaskDag` over canonical entities using `TaskReflectionContext`.
    pub fn execute_dag(
        &mut self,
        plan_id: &str,
        mode: crate::reflection::contracts::ReflectionExecutionMode,
        dag: &TaskDag,
        context: &TaskReflectionContext,
        entities: &mut Vec<CanonicalEntity>,
    ) -> Result<ReflectionResult, String> {
        let start = Instant::now();
        let correlation_id = Uuid::new_v4();
        let stages = dag.compute_stages()?;

        let mut start_stage_index = 0;
        let mut completed_task_ids = Vec::new();
        let mut modified_entity_ids = Vec::new();

        // Check if an existing checkpoint exists for resumable recovery
        if let Some(cp) = self.checkpoint_runtime.load_checkpoint(plan_id)? {
            start_stage_index = RecoveryRuntime::prepare_resumption(&cp)?;
            completed_task_ids = cp.completed_task_ids;
            modified_entity_ids = cp.modified_entity_ids;

            if let Some(bus) = &self.event_bus {
                let evt = ReflectionRuntimeEvent::RecoveryStarted {
                    plan_id: plan_id.to_string(),
                    resuming_stage_index: start_stage_index,
                    timestamp_ms: context.clock_timestamp_ms,
                };
                bus.publish(brain_events::ReflectionEventEnvelope::new(
                    plan_id,
                    None,
                    correlation_id,
                    context.clock_timestamp_ms,
                    evt,
                ));
            }
        }

        let mut task_reports = Vec::new();

        #[allow(clippy::needless_range_loop)]
        for stage_index in start_stage_index..stages.len() {
            if context.cancellation_token.is_cancelled() {
                return Err("Reflection execution aborted by cancellation token".to_string());
            }

            let stage_task_ids = &stages[stage_index];

            for task_id in stage_task_ids {
                if let Some(node) = dag.get_node(task_id) {
                    if let Some(bus) = &self.event_bus {
                        let evt = ReflectionRuntimeEvent::TaskStarted {
                            plan_id: plan_id.to_string(),
                            stage_index,
                            task_id: task_id.clone(),
                            timestamp_ms: context.clock_timestamp_ms,
                        };
                        bus.publish(brain_events::ReflectionEventEnvelope::new(
                            plan_id,
                            Some(task_id.clone()),
                            correlation_id,
                            context.clock_timestamp_ms,
                            evt,
                        ));
                    }

                    let mut attempts = 0;
                    loop {
                        attempts += 1;
                        let report = node.task.execute(entities);

                        // Retry check if no items processed despite non-empty input
                        if report.items_processed == 0
                            && !entities.is_empty()
                            && (attempts as u32) < self.retry_policy.max_attempts
                        {
                            if let Some(bus) = &self.event_bus {
                                let evt = ReflectionRuntimeEvent::TaskRetried {
                                    plan_id: plan_id.to_string(),
                                    stage_index,
                                    task_id: task_id.clone(),
                                    attempt: attempts as u32,
                                    error_message: "No items processed".to_string(),
                                    timestamp_ms: context.clock_timestamp_ms,
                                };
                                bus.publish(brain_events::ReflectionEventEnvelope::new(
                                    plan_id,
                                    Some(task_id.clone()),
                                    correlation_id,
                                    context.clock_timestamp_ms,
                                    evt,
                                ));
                            }
                            continue;
                        }

                        if report.changes_applied > 0 {
                            for e in entities.iter() {
                                if !modified_entity_ids.contains(&e.id) {
                                    modified_entity_ids.push(e.id);
                                }
                            }
                        }

                        if let Some(bus) = &self.event_bus {
                            let evt = ReflectionRuntimeEvent::TaskCompleted {
                                plan_id: plan_id.to_string(),
                                stage_index,
                                task_id: task_id.clone(),
                                duration_ms: report.duration.as_millis() as u64,
                                changes_applied: report.changes_applied,
                                timestamp_ms: context.clock_timestamp_ms,
                            };
                            bus.publish(brain_events::ReflectionEventEnvelope::new(
                                plan_id,
                                Some(task_id.clone()),
                                correlation_id,
                                context.clock_timestamp_ms,
                                evt,
                            ));
                        }

                        completed_task_ids.push(task_id.clone());
                        task_reports.push(report);
                        break;
                    }
                }
            }

            // Capture lightweight stage checkpoint
            let cp = Checkpoint::new(
                plan_id,
                stage_index,
                completed_task_ids.clone(),
                modified_entity_ids.clone(),
                context.clock_timestamp_ms,
            );
            self.checkpoint_runtime.save_checkpoint(cp);

            if let Some(bus) = &self.event_bus {
                let evt = ReflectionRuntimeEvent::CheckpointCreated {
                    plan_id: plan_id.to_string(),
                    stage_index,
                    modified_entity_count: modified_entity_ids.len(),
                    timestamp_ms: context.clock_timestamp_ms,
                };
                bus.publish(brain_events::ReflectionEventEnvelope::new(
                    plan_id,
                    None,
                    correlation_id,
                    context.clock_timestamp_ms,
                    evt,
                ));
            }
        }

        Ok(ReflectionResult {
            plan_id: plan_id.to_string(),
            execution_mode: mode,
            modified_entity_ids,
            task_reports,
            diagnostics: Vec::new(),
            total_duration: start.elapsed(),
        })
    }
}
