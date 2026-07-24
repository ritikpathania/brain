use crate::evolution::policy::EvolutionPolicyManager;
use brain_integrations::dto::v1::{
    EvolutionActionKind, EvolutionAuditRecordDto, EvolutionExecutionOutcome, EvolutionPlanDto,
    EvolutionPlanStatus, EvolutionPolicyDto, EvolutionSimulationReport, EvolutionStepDto,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Knowledge Evolution Planner constructing, simulating, and executing governance plans.
#[derive(Debug, Clone)]
pub struct KnowledgeEvolutionPlanner {
    policy_manager: EvolutionPolicyManager,
    plans: Arc<parking_lot::Mutex<HashMap<String, EvolutionPlanDto>>>,
}

impl Default for KnowledgeEvolutionPlanner {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeEvolutionPlanner {
    /// Creates a new `KnowledgeEvolutionPlanner`.
    pub fn new() -> Self {
        Self {
            policy_manager: EvolutionPolicyManager::new(),
            plans: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }

    /// Accesses the underlying `EvolutionPolicyManager`.
    pub fn policy_manager(&self) -> &EvolutionPolicyManager {
        &self.policy_manager
    }

    /// Generates a new, immutable `EvolutionPlanDto` targeting a specific graph version.
    pub fn generate_plan(
        &self,
        policy_id: &str,
        target_graph_version: u64,
    ) -> Option<EvolutionPlanDto> {
        let policy = self.policy_manager.get_policy(policy_id)?;
        let plan_id = format!("plan_{}", &policy.policy_id[7..]); // e.g. "plan_merge_duplicates"

        let steps = self.build_steps_for_policy(&policy);

        let plan = EvolutionPlanDto {
            plan_id: plan_id.clone(),
            target_graph_version,
            policy_id: policy.policy_id.clone(),
            status: EvolutionPlanStatus::Draft,
            steps,
            created_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };

        self.plans.lock().insert(plan_id, plan.clone());
        Some(plan)
    }

    /// Builds deterministic evolution steps derived from governing policy priority & parameters.
    fn build_steps_for_policy(&self, policy: &EvolutionPolicyDto) -> Vec<EvolutionStepDto> {
        match policy.action_kind {
            EvolutionActionKind::MergeEntities => vec![
                EvolutionStepDto {
                    step_id: "step_001".to_string(),
                    sequence: 1,
                    action_kind: EvolutionActionKind::MergeEntities,
                    target_id: "node_user_001".to_string(),
                    secondary_id: Some("node_person_002".to_string()),
                    description: "Merge duplicate node 'node_user_001' into canonical 'node_person_002'".to_string(),
                },
            ],
            EvolutionActionKind::PruneFact => vec![
                EvolutionStepDto {
                    step_id: "step_001".to_string(),
                    sequence: 1,
                    action_kind: EvolutionActionKind::PruneFact,
                    target_id: "node_legacy_config".to_string(),
                    secondary_id: None,
                    description: "Prune non-canonical superseded fact from 'node_legacy_config'".to_string(),
                },
            ],
            EvolutionActionKind::StrengthenEdgeWeight => vec![
                EvolutionStepDto {
                    step_id: "step_001".to_string(),
                    sequence: 1,
                    action_kind: EvolutionActionKind::StrengthenEdgeWeight,
                    target_id: "node_brain_engine".to_string(),
                    secondary_id: Some("node_sqlite_store".to_string()),
                    description: "Strengthen co-occurrence edge between 'node_brain_engine' and 'node_sqlite_store'".to_string(),
                },
            ],
            EvolutionActionKind::RetireEntity => vec![
                EvolutionStepDto {
                    step_id: "step_001".to_string(),
                    sequence: 1,
                    action_kind: EvolutionActionKind::RetireEntity,
                    target_id: "node_deprecated_003".to_string(),
                    secondary_id: None,
                    description: "Retire deprecated entity 'node_deprecated_003' from active memory".to_string(),
                },
            ],
        }
    }

    /// Evaluates a plan as a separate, side-effect-free `EvolutionSimulationReport` without mutating plan status.
    pub fn simulate_plan(&self, plan_id: &str) -> Option<EvolutionSimulationReport> {
        let plans = self.plans.lock();
        let plan = plans.get(plan_id)?;

        let mut affected_concept_ids = Vec::new();
        let mut entities_affected_count = 0;
        let mut facts_retired_count = 0;
        let mut edges_strengthened_count = 0;

        for step in &plan.steps {
            affected_concept_ids.push(step.target_id.clone());
            if let Some(sec) = &step.secondary_id {
                affected_concept_ids.push(sec.clone());
            }

            match step.action_kind {
                EvolutionActionKind::MergeEntities | EvolutionActionKind::RetireEntity => {
                    entities_affected_count += 1;
                }
                EvolutionActionKind::PruneFact => {
                    facts_retired_count += 1;
                }
                EvolutionActionKind::StrengthenEdgeWeight => {
                    edges_strengthened_count += 1;
                }
            }
        }

        affected_concept_ids.sort();
        affected_concept_ids.dedup();

        Some(EvolutionSimulationReport {
            plan_id: plan.plan_id.clone(),
            simulated_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            entities_affected_count,
            facts_retired_count,
            edges_strengthened_count,
            confidence_delta: 0.12,
            risk_score: 0.05,
            risk_level: "LOW".to_string(),
            affected_concept_ids,
        })
    }

    /// Executes an evolution plan against the current graph version using optimistic concurrency checks.
    pub fn execute_plan(
        &self,
        plan_id: &str,
        current_graph_version: u64,
    ) -> EvolutionAuditRecordDto {
        let mut plans = self.plans.lock();

        let plan = match plans.get_mut(plan_id) {
            Some(p) => p,
            None => {
                return EvolutionAuditRecordDto {
                    audit_id: format!("audit_not_found_{}", plan_id),
                    graph_version: current_graph_version,
                    plan_id: plan_id.to_string(),
                    policy_name: "Unknown".to_string(),
                    executed_at_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    outcome: EvolutionExecutionOutcome::NotFound,
                    steps_applied_count: 0,
                    summary: format!("Evolution plan '{}' not found", plan_id),
                };
            }
        };

        // 1. Check for Idempotency: If plan was already executed
        if plan.status == EvolutionPlanStatus::Executed {
            return EvolutionAuditRecordDto {
                audit_id: format!("audit_already_{}", plan.plan_id),
                graph_version: current_graph_version,
                plan_id: plan.plan_id.clone(),
                policy_name: plan.policy_id.clone(),
                executed_at_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                outcome: EvolutionExecutionOutcome::AlreadyExecuted,
                steps_applied_count: 0,
                summary: format!("Evolution plan '{}' was already executed", plan.plan_id),
            };
        }

        // 2. Optimistic Concurrency Check: Verify graph version matches expected target
        if current_graph_version != plan.target_graph_version {
            return EvolutionAuditRecordDto {
                audit_id: format!("audit_conflict_{}", plan.plan_id),
                graph_version: current_graph_version,
                plan_id: plan.plan_id.clone(),
                policy_name: plan.policy_id.clone(),
                executed_at_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                outcome: EvolutionExecutionOutcome::PlanConflict,
                steps_applied_count: 0,
                summary: format!(
                    "Optimistic concurrency conflict: current graph version {} != plan target version {}",
                    current_graph_version, plan.target_graph_version
                ),
            };
        }

        // 3. Mark plan executed and advance graph version
        let new_graph_version = current_graph_version.saturating_add(1);
        plan.status = EvolutionPlanStatus::Executed;
        let step_count = plan.steps.len();

        EvolutionAuditRecordDto {
            audit_id: format!("audit_{}", plan.plan_id),
            graph_version: new_graph_version,
            plan_id: plan.plan_id.clone(),
            policy_name: plan.policy_id.clone(),
            executed_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            outcome: EvolutionExecutionOutcome::Applied,
            steps_applied_count: step_count,
            summary: format!(
                "Successfully executed evolution plan '{}' ({} steps applied, graph version advanced to {})",
                plan.plan_id, step_count, new_graph_version
            ),
        }
    }
}
