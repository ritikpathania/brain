//! `ExecutionPlanner` compiling `TaskPlan` into parallel `ExecutionPlan` stage graphs (Phase 7 Milestone 7.3).

use crate::planning::execution_plan::{
    ExecutionPlan, ExecutionPlanId, ExecutionPlanningError, ExecutionPlanningPolicy, ExecutionStage,
};
use crate::planning::models::{TaskId, TaskPlan};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Deterministic execution planner grouping `TaskPlan` steps into parallel `ExecutionStage`s.
#[derive(Debug, Clone, Default)]
pub struct ExecutionPlanner;

impl ExecutionPlanner {
    /// Instantiates a new `ExecutionPlanner`.
    pub fn new() -> Self {
        Self
    }

    /// Partitions `TaskPlan` nodes into parallel `ExecutionStage` instances deterministically.
    pub fn plan_execution(
        &self,
        plan: &TaskPlan,
        policy: &ExecutionPlanningPolicy,
    ) -> Result<ExecutionPlan, ExecutionPlanningError> {
        if plan.task_graph.nodes.is_empty() {
            return Err(ExecutionPlanningError::InvalidTaskGraph(
                "TaskPlan graph has no nodes".to_string(),
            ));
        }

        // Build in-degree map and adjacency list
        let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
        let mut adj: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
        let mut all_task_ids = HashSet::new();

        for node in &plan.task_graph.nodes {
            in_degree.insert(node.task_id, 0);
            adj.insert(node.task_id, Vec::new());
            all_task_ids.insert(node.task_id);
        }

        for edge in &plan.task_graph.edges {
            if !all_task_ids.contains(&edge.source) || !all_task_ids.contains(&edge.target) {
                return Err(ExecutionPlanningError::InvalidTaskGraph(
                    "TaskDependencyEdge references non-existent TaskId".to_string(),
                ));
            }
            *in_degree.entry(edge.target).or_insert(0) += 1;
            adj.entry(edge.source).or_default().push(edge.target);
        }

        // Deterministic level-by-level topological stage assignment
        let mut stages = Vec::new();
        let mut processed_tasks = HashSet::new();

        // Level 0: nodes with in-degree == 0
        let mut current_level: Vec<TaskId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        if current_level.is_empty() {
            return Err(ExecutionPlanningError::DependencyCycle(
                "Cycle detected; no initial tasks with zero in-degree found".to_string(),
            ));
        }

        let mut stage_index = 0;

        while !current_level.is_empty() {
            // Sort deterministically to enforce PlanExecution(plan) == PlanExecution(plan)
            current_level.sort_by_key(|id| id.0);

            for &id in &current_level {
                processed_tasks.insert(id);
            }

            let stage_cost = current_level.len() as f32 * 1.0;
            stages.push(ExecutionStage {
                stage_index,
                parallel_tasks: current_level.clone(),
                estimated_cost: stage_cost,
                barrier_kind: policy.default_barrier,
            });

            stage_index += 1;

            // Compute next level
            let mut next_level = Vec::new();
            for &u in &current_level {
                if let Some(neighbors) = adj.get(&u) {
                    for &v in neighbors {
                        if let Some(deg) = in_degree.get_mut(&v) {
                            *deg -= 1;
                            if *deg == 0 {
                                next_level.push(v);
                            }
                        }
                    }
                }
            }

            current_level = next_level;
        }

        // Verify invariant: every task in TaskPlan appears in ExecutionPlan
        if processed_tasks.len() != all_task_ids.len() {
            return Err(ExecutionPlanningError::DependencyCycle(
                "Cycle detected; some tasks were never scheduled".to_string(),
            ));
        }

        Ok(ExecutionPlan {
            execution_plan_id: ExecutionPlanId(Uuid::new_v4()),
            task_plan_id: plan.plan_id,
            stages,
            timestamp_ms: plan.timestamp_ms + 100,
        })
    }
}
