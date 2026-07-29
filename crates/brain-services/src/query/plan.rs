//! Declarative execution plan structures for the Knowledge Query Engine.

use crate::compiler::EntityId;
use crate::query::ast::TemporalRange;
use brain_domain::RelationId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Strongly-typed 0-indexed identifier for an execution step within an `ExecutionPlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionStepId(pub usize);

impl std::fmt::Display for ExecutionStepId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "step_{}", self.0)
    }
}

/// Declarative step parameter for lexical text pattern match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStep {
    /// Lexical pattern string.
    pub pattern: String,
}

/// Declarative step parameter for semantic embedding similarity match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticStep {
    /// Semantic prompt text.
    pub prompt: String,
}

/// Declarative step parameter for relationship hop traversal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphStep {
    /// Strongly-typed relation identifier.
    pub relation_kind: RelationId,
    /// Strongly-typed target entity identifier.
    pub target_id: EntityId,
}

/// Declarative step parameter for temporal range constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalStep {
    /// Time range bounds.
    pub range: TemporalRange,
}

/// Declarative execution step enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionStep {
    /// Lexical full-text pattern match step.
    Text(TextStep),
    /// Semantic embedding similarity match step.
    Semantic(SemanticStep),
    /// Relationship hop graph traversal step.
    Graph(GraphStep),
    /// Temporal range constraint step.
    Temporal(TemporalStep),
}

/// First-class inspectable execution plan compiled from a `KnowledgeQuery` AST.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Unique plan execution UUID.
    pub query_id: Uuid,
    /// Ordered sequence of identified execution steps.
    pub steps: Vec<(ExecutionStepId, ExecutionStep)>,
}

impl ExecutionPlan {
    /// Instantiates a new `ExecutionPlan`.
    pub fn new(query_id: Uuid) -> Self {
        Self {
            query_id,
            steps: Vec::new(),
        }
    }

    /// Appends a new step into the plan and returns its `ExecutionStepId`.
    pub fn add_step(&mut self, step: ExecutionStep) -> ExecutionStepId {
        let step_id = ExecutionStepId(self.steps.len());
        self.steps.push((step_id, step));
        step_id
    }
}
