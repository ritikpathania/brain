//! Domain models, strong identifiers, and invariant validators for runtime reasoning execution plans.

use crate::memory::MemoryFilter;
use crate::DomainError;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Strongly-typed identifier for an execution plan step.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct PlanStepId(pub u32);

impl PlanStepId {
    /// Instantiates a new strongly-typed step identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the underlying numeric identifier value.
    pub fn value(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for PlanStepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "step-{}", self.0)
    }
}

/// Advisory complexity rating for a reasoning plan step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PlanStepComplexity {
    /// Low execution cost / simple lookup step.
    Low,
    /// Medium complexity graph or memory query.
    Medium,
    /// High complexity analysis, evidence collection, or synthesis.
    High,
}

impl PlanStepComplexity {
    /// Returns user-facing badge text.
    pub fn badge_text(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

/// Capability-oriented intent classifications for reasoning plan steps.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReasoningPlanStepKind {
    /// Search engine or message search step.
    Search {
        /// Query string to search.
        query: String,
    },
    /// Memory stewardship query step.
    QueryMemory {
        /// Memory filter criteria.
        filter: MemoryFilter,
    },
    /// Shared entity inspection step.
    InspectEntity {
        /// Entity or node identifier.
        entity_id: String,
    },
    /// Relationship adjacency traversal step.
    TraverseRelationships {
        /// Target entity identifier.
        entity_id: String,
    },
    /// Evidence collection and aggregation step.
    CollectEvidence {
        /// Contributing step identifiers.
        step_ids: Vec<PlanStepId>,
    },
    /// Terminal response synthesis step.
    SynthesizeResponse,
}

/// A discrete dependency-linked step in an execution plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningPlanStep {
    /// Strongly-typed step identifier.
    pub id: PlanStepId,
    /// Capability-oriented step kind.
    pub kind: ReasoningPlanStepKind,
    /// Human-readable step description.
    pub description: String,
    /// Step dependencies that must complete before this step can execute.
    pub depends_on: Vec<PlanStepId>,
    /// Optional advisory complexity rating.
    pub complexity: Option<PlanStepComplexity>,
}

impl ReasoningPlanStep {
    /// Creates a new reasoning plan step.
    pub fn new(
        id: PlanStepId,
        kind: ReasoningPlanStepKind,
        description: impl Into<String>,
        depends_on: Vec<PlanStepId>,
        complexity: Option<PlanStepComplexity>,
    ) -> Self {
        Self {
            id,
            kind,
            description: description.into(),
            depends_on,
            complexity,
        }
    }
}

/// Immutable pure domain aggregate representing a validated execution plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionPlan {
    /// Unique plan identifier string.
    pub id: String,
    /// Originating user query or command prompt.
    pub user_query: String,
    /// Dependency-linked steps composing the plan DAG.
    pub steps: Vec<ReasoningPlanStep>,
}

impl ExecutionPlan {
    /// Instantiates and validates a new `ExecutionPlan`.
    pub fn new(
        id: impl Into<String>,
        user_query: impl Into<String>,
        steps: Vec<ReasoningPlanStep>,
    ) -> Result<Self, DomainError> {
        let plan = Self {
            id: id.into(),
            user_query: user_query.into(),
            steps,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Performs complete structural invariant validation on the execution plan DAG.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.steps.is_empty() {
            return Err(DomainError::ValidationError {
                message: "ExecutionPlan cannot be empty and must contain at least one step"
                    .to_string(),
                rule_id: Some("VAL-PLAN-001".to_string()),
            });
        }

        // 1. Step ID uniqueness check
        let mut step_ids = HashSet::new();
        for step in &self.steps {
            if !step_ids.insert(step.id) {
                return Err(DomainError::ValidationError {
                    message: format!("Duplicate step ID found in ExecutionPlan: {}", step.id),
                    rule_id: Some("VAL-PLAN-002".to_string()),
                });
            }
        }

        // 2. Dangling dependency reference check
        for step in &self.steps {
            for dep_id in &step.depends_on {
                if !step_ids.contains(dep_id) {
                    return Err(DomainError::ValidationError {
                        message: format!(
                            "Step {} depends on non-existent step {}",
                            step.id, dep_id
                        ),
                        rule_id: Some("VAL-PLAN-003".to_string()),
                    });
                }
            }

            // Check CollectEvidence referencing existing step IDs
            if let ReasoningPlanStepKind::CollectEvidence {
                step_ids: ref ref_steps,
            } = step.kind
            {
                for ref_id in ref_steps {
                    if !step_ids.contains(ref_id) {
                        return Err(DomainError::ValidationError {
                            message: format!(
                                "CollectEvidence step {} references non-existent step {}",
                                step.id, ref_id
                            ),
                            rule_id: Some("VAL-PLAN-004".to_string()),
                        });
                    }
                }
            }
        }

        // 3. Terminal step existence check (must contain at least one SynthesizeResponse step)
        let has_terminal_step = self
            .steps
            .iter()
            .any(|step| matches!(step.kind, ReasoningPlanStepKind::SynthesizeResponse));
        if !has_terminal_step {
            return Err(DomainError::ValidationError {
                message: "ExecutionPlan must contain at least one terminal SynthesizeResponse step"
                    .to_string(),
                rule_id: Some("VAL-PLAN-005".to_string()),
            });
        }

        // 4. Cycle detection pass
        self.validate_acyclic()?;

        Ok(())
    }

    /// Validates that the plan steps form a valid Directed Acyclic Graph (DAG).
    fn validate_acyclic(&self) -> Result<(), DomainError> {
        let mut in_degree: HashMap<PlanStepId, usize> = HashMap::new();
        let mut adj: HashMap<PlanStepId, Vec<PlanStepId>> = HashMap::new();

        for step in &self.steps {
            in_degree.entry(step.id).or_insert(0);
            adj.entry(step.id).or_default();
        }

        for step in &self.steps {
            for dep_id in &step.depends_on {
                adj.entry(*dep_id).or_default().push(step.id);
                *in_degree.entry(step.id).or_default() += 1;
            }
        }

        let mut queue: Vec<PlanStepId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut visited_count = 0;

        while let Some(u) = queue.pop() {
            visited_count += 1;
            if let Some(neighbors) = adj.get(&u) {
                for &v in neighbors {
                    if let Some(deg) = in_degree.get_mut(&v) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(v);
                        }
                    }
                }
            }
        }

        if visited_count != self.steps.len() {
            return Err(DomainError::ValidationError {
                message: "Cyclic dependency detected in ExecutionPlan step DAG".to_string(),
                rule_id: Some("VAL-PLAN-006".to_string()),
            });
        }

        Ok(())
    }

    /// Looks up a plan step by its strongly-typed identifier.
    pub fn get_step(&self, id: PlanStepId) -> Option<&ReasoningPlanStep> {
        self.steps.iter().find(|step| step.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_execution_plan_validation() {
        let step1 = ReasoningPlanStep::new(
            PlanStepId::new(1),
            ReasoningPlanStepKind::Search {
                query: "retrieval".to_string(),
            },
            "Search for retrieval engine context",
            vec![],
            Some(PlanStepComplexity::Low),
        );
        let step2 = ReasoningPlanStep::new(
            PlanStepId::new(2),
            ReasoningPlanStepKind::SynthesizeResponse,
            "Synthesize response findings",
            vec![PlanStepId::new(1)],
            Some(PlanStepComplexity::High),
        );

        let plan = ExecutionPlan::new("plan_1", "How does retrieval work?", vec![step1, step2]);
        assert!(plan.is_ok());
    }

    #[test]
    fn test_duplicate_step_id_rejected() {
        let step1 = ReasoningPlanStep::new(
            PlanStepId::new(1),
            ReasoningPlanStepKind::Search {
                query: "retrieval".to_string(),
            },
            "Search for retrieval engine context",
            vec![],
            None,
        );
        let step2 = ReasoningPlanStep::new(
            PlanStepId::new(1),
            ReasoningPlanStepKind::SynthesizeResponse,
            "Synthesize response findings",
            vec![],
            None,
        );

        let plan = ExecutionPlan::new("plan_dup", "query", vec![step1, step2]);
        assert!(plan.is_err());
    }

    #[test]
    fn test_missing_dependency_is_rejected() {
        let step1 = ReasoningPlanStep::new(
            PlanStepId::new(1),
            ReasoningPlanStepKind::SynthesizeResponse,
            "Synthesize response findings",
            vec![PlanStepId::new(999)], // Step 999 does not exist!
            None,
        );

        let plan = ExecutionPlan::new("plan_missing_dep", "query", vec![step1]);
        assert!(plan.is_err());
    }

    #[test]
    fn test_cyclic_dependency_is_rejected() {
        let step1 = ReasoningPlanStep::new(
            PlanStepId::new(1),
            ReasoningPlanStepKind::Search {
                query: "a".to_string(),
            },
            "Step 1",
            vec![PlanStepId::new(2)],
            None,
        );
        let step2 = ReasoningPlanStep::new(
            PlanStepId::new(2),
            ReasoningPlanStepKind::SynthesizeResponse,
            "Step 2",
            vec![PlanStepId::new(1)],
            None,
        );

        let plan = ExecutionPlan::new("plan_cycle", "query", vec![step1, step2]);
        assert!(plan.is_err());
    }
}
