//! `GoalDecomposer` analyzing `GoalIntent` against `KnowledgeRuntime` context to produce `PlanningIR` (Phase 7 Milestone 7.1).

use crate::planning::models::{CapabilityId, GoalIntent, PlanningIR, TaskCandidate, TaskId};
use crate::query::context::QueryContextProvider;
use crate::query::pipeline::QueryPipeline;
use uuid::Uuid;

/// Goal decomposer producing intermediate `PlanningIR` representations.
#[derive(Default)]
pub struct GoalDecomposer {
    query_pipeline: QueryPipeline,
}

impl GoalDecomposer {
    /// Instantiates a new `GoalDecomposer`.
    pub fn new() -> Self {
        Self {
            query_pipeline: QueryPipeline::new(),
        }
    }

    /// Decomposes a `GoalIntent` into `PlanningIR` by retrieving context from `QueryContextProvider`.
    pub fn decompose(&self, goal: &GoalIntent, ctx: &dyn QueryContextProvider) -> PlanningIR {
        let query_result = self.query_pipeline.execute(&goal.context_query, ctx);

        let mut candidates = Vec::new();
        let mut candidate_ids = Vec::new();

        for (idx, candidate) in query_result.candidates.iter().enumerate() {
            let task_id = TaskId(Uuid::new_v4());
            candidate_ids.push(task_id);

            candidates.push(TaskCandidate {
                task_id,
                description: format!(
                    "Execute task step {} for entity '{}'",
                    idx + 1,
                    candidate.entity_id
                ),
                required_capabilities: vec![CapabilityId("capability_execution".to_string())],
                evidence: vec![],
                confidence: candidate.score,
            });
        }

        if candidates.is_empty() {
            let fallback_id = TaskId(Uuid::new_v4());
            candidate_ids.push(fallback_id);
            candidates.push(TaskCandidate {
                task_id: fallback_id,
                description: format!("Fulfill goal intent: {}", goal.description),
                required_capabilities: vec![CapabilityId("capability_execution".to_string())],
                evidence: vec![],
                confidence: 1.0,
            });
        }

        PlanningIR {
            goal_id: goal.goal_id,
            candidates,
            alternative_decompositions: vec![candidate_ids],
            constraints: goal.constraints.clone(),
            priority: goal.priority,
        }
    }
}
