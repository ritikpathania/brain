//! `TaskPlanCompiler` synthesizing compiled immutable `TaskPlan` artifacts from `PlanningIR` (Phase 7 Milestone 7.1).

use crate::planning::models::{
    PlanId, PlanningIR, TaskDependencyEdge, TaskGraph, TaskPlan, TaskStep,
};
use uuid::Uuid;

/// Compiler producing immutable `TaskPlan` artifacts from intermediate `PlanningIR`.
#[derive(Debug, Clone, Default)]
pub struct TaskPlanCompiler;

impl TaskPlanCompiler {
    /// Instantiates a new `TaskPlanCompiler`.
    pub fn new() -> Self {
        Self
    }

    /// Compiles `PlanningIR` into an immutable `TaskPlan`.
    pub fn compile(&self, ir: &PlanningIR) -> TaskPlan {
        let mut graph = TaskGraph::new();

        for candidate in &ir.candidates {
            graph.nodes.push(TaskStep {
                task_id: candidate.task_id,
                description: candidate.description.clone(),
                required_capabilities: candidate.required_capabilities.clone(),
                confidence: candidate.confidence,
            });
        }

        if graph.nodes.len() >= 2 {
            for i in 0..(graph.nodes.len() - 1) {
                graph.edges.push(TaskDependencyEdge {
                    source: graph.nodes[i].task_id,
                    target: graph.nodes[i + 1].task_id,
                });
            }
        }

        TaskPlan {
            plan_id: PlanId(Uuid::new_v4()),
            goal_id: ir.goal_id,
            task_graph: graph,
            priority: ir.priority,
            timestamp_ms: 7500,
        }
    }
}
