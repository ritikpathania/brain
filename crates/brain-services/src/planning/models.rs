//! Domain models for Goal Decomposition, Planning IR, and Task Plan Compilation (Phase 7 Milestone 7.1).
//!
//! ### Architectural Invariants:
//! 1. `PlanningIR` is the **ONLY** mutable planning representation and optimization boundary.
//! 2. `TaskPlan` is a compiled **immutable** executable artifact; once compiled, it cannot be mutated.
//! 3. `GoalValidator` is strictly **pure** and deterministic.
//! 4. `PlanningRuntime` is a thin orchestrator only.
//! 5. Milestone 7.1 stops strictly at compiled `TaskPlan` (no execution logic).

use crate::query::ast::KnowledgeQuery;
use crate::reasoning::models::EvidenceRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Strongly-typed goal identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GoalId(pub Uuid);

impl std::fmt::Display for GoalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "goal_{}", self.0)
    }
}

/// Strongly-typed task step identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "task_{}", self.0)
    }
}

/// Strongly-typed plan identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlanId(pub Uuid);

impl std::fmt::Display for PlanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "task_plan_{}", self.0)
    }
}

/// Strongly-typed capability identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CapabilityId(pub String);

impl std::fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cap_{}", self.0)
    }
}

/// Goal execution priority classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    /// Low execution priority.
    Low,
    /// Normal execution priority.
    Normal,
    /// High execution priority.
    High,
    /// Critical execution priority.
    Critical,
}

/// Domain constraint classification for goal decomposition and plan validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    /// Required capability that must be present.
    MandatoryCapability(CapabilityId),
    /// Time window upper bound in milliseconds.
    TimeWindowMs(u64),
    /// Cost upper bound constraint.
    MaxCost(f32),
}

/// Declarative goal intent input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalIntent {
    /// Unique goal identifier.
    pub goal_id: GoalId,
    /// Goal description text.
    pub description: String,
    /// Knowledge context query for evidence retrieval.
    pub context_query: KnowledgeQuery,
    /// Mandatory domain constraints.
    pub constraints: Vec<Constraint>,
    /// Goal execution priority.
    pub priority: Priority,
}

/// Candidate task node produced during goal decomposition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskCandidate {
    /// Unique candidate task ID.
    pub task_id: TaskId,
    /// Task step description.
    pub description: String,
    /// Required capability IDs.
    pub required_capabilities: Vec<CapabilityId>,
    /// Supporting evidence references from KnowledgeRuntime.
    pub evidence: Vec<EvidenceRef>,
    /// Confidence score [0.0..1.0].
    pub confidence: f32,
}

/// Intermediate Representation (`PlanningIR`) — the ONLY mutable representation for planning optimization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningIR {
    /// Target goal ID.
    pub goal_id: GoalId,
    /// Candidate task steps.
    pub candidates: Vec<TaskCandidate>,
    /// Alternative decomposition paths (task ID sequences).
    pub alternative_decompositions: Vec<Vec<TaskId>>,
    /// Applied domain constraints.
    pub constraints: Vec<Constraint>,
    /// Goal execution priority.
    pub priority: Priority,
}

/// Single executable step in a compiled `TaskPlan`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskStep {
    /// Unique task ID.
    pub task_id: TaskId,
    /// Task description.
    pub description: String,
    /// Required capability IDs.
    pub required_capabilities: Vec<CapabilityId>,
    /// Execution confidence score.
    pub confidence: f32,
}

/// Directed dependency edge between tasks (source task must complete before target task).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskDependencyEdge {
    /// Prerequisite source task ID.
    pub source: TaskId,
    /// Dependent target task ID.
    pub target: TaskId,
}

/// Canonical task dependency graph representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TaskGraph {
    /// List of task step nodes.
    pub nodes: Vec<TaskStep>,
    /// Directed dependency edges.
    pub edges: Vec<TaskDependencyEdge>,
}

impl TaskGraph {
    /// Instantiates a new `TaskGraph`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Derives topological execution ordering on demand. Returns an error if a cycle is detected.
    pub fn topological_sort(&self) -> Result<Vec<TaskId>, String> {
        let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
        let mut adj: HashMap<TaskId, Vec<TaskId>> = HashMap::new();

        for node in &self.nodes {
            in_degree.insert(node.task_id, 0);
            adj.insert(node.task_id, Vec::new());
        }

        for edge in &self.edges {
            *in_degree.entry(edge.target).or_insert(0) += 1;
            adj.entry(edge.source).or_default().push(edge.target);
        }

        let mut queue: Vec<TaskId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut order = Vec::new();

        while let Some(u) = queue.pop() {
            order.push(u);

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

        if order.len() != self.nodes.len() {
            Err("Cycle detected in TaskGraph dependencies".to_string())
        } else {
            Ok(order)
        }
    }
}

/// Compiled immutable executable `TaskPlan` artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskPlan {
    /// Unique plan ID.
    pub plan_id: PlanId,
    /// Target goal ID.
    pub goal_id: GoalId,
    /// Canonical task dependency graph.
    pub task_graph: TaskGraph,
    /// Priority level.
    pub priority: Priority,
    /// Compilation timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Classification kind for planning validation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanningValidationKind {
    /// Dependency cycle detected in TaskGraph.
    DependencyCycle,
    /// Required capability is missing.
    MissingCapability,
    /// Orphan task node with no connections.
    OrphanTask,
    /// Duplicate task ID found.
    DuplicateTask,
    /// Unreachable task node.
    UnreachableTask,
    /// Domain constraint violation.
    ConstraintViolation,
}

/// Validation error item emitted by `GoalValidator`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningValidationError {
    /// Classification kind.
    pub kind: PlanningValidationKind,
    /// Optional target task ID.
    pub task_id: Option<TaskId>,
    /// Machine and human readable details.
    pub details: String,
}

/// Pure validation report emitted by `GoalValidator`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningValidationReport {
    /// Flag indicating whether the compiled plan is valid.
    pub is_valid: bool,
    /// List of validation errors discovered.
    pub errors: Vec<PlanningValidationError>,
}
