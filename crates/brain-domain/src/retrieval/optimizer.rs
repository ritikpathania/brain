use crate::retrieval::models::{
    LogicalRetrievalPlan, LogicalStep, PhysicalRetrievalPlan, PhysicalStep, EstimatedCost, CostHeuristics
};

/// Optimizer translating Logical Retrieval Plans to Physical ones.
pub struct PlanOptimizer;

impl PlanOptimizer {
    /// Translates a logical plan into an optimized, executable physical plan using cost heuristics.
    pub fn optimize(&self, plan: LogicalRetrievalPlan, heuristics: &CostHeuristics) -> PhysicalRetrievalPlan {
        let mut physical_steps = Vec::new();
        let mut vector_cost = 0.0;
        let mut keyword_cost = 0.0;
        let mut expansion_cost = 0.0;

        for step in plan.steps {
            match step {
                LogicalStep::VectorRetrieve { query } => {
                    if !query.trim().is_empty() {
                        physical_steps.push(PhysicalStep::VectorRetrieve { query });
                        vector_cost += heuristics.weights.vector_weight;
                    }
                }
                LogicalStep::KeywordRetrieve { query } => {
                    if !query.trim().is_empty() {
                        physical_steps.push(PhysicalStep::KeywordRetrieve { query });
                        keyword_cost += heuristics.weights.keyword_weight;
                    }
                }
                LogicalStep::ExpandNeighbors { source_nodes, policy } => {
                    physical_steps.push(PhysicalStep::ExpandNeighbors { source_nodes, policy });
                    expansion_cost += heuristics.weights.expansion_weight;
                }
            }
        }

        let fusion_cost = if physical_steps.len() > 1 { heuristics.weights.fusion_weight } else { 0.0 };
        let ranking_cost = if !physical_steps.is_empty() { heuristics.weights.ranking_weight } else { 0.0 };

        let cost = EstimatedCost {
            vector_cost,
            keyword_cost,
            expansion_cost,
            fusion_cost,
            ranking_cost,
        };

        PhysicalRetrievalPlan {
            physical_steps,
            cost,
            heuristics_version: heuristics.metadata.version,
        }
    }
}
