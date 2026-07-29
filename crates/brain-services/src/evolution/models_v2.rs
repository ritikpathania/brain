//! Domain models for Knowledge Evolution Planning, Validation, and Transactional Execution (Phase 6 Milestone 6.2).
//!
//! ### Architectural Invariants:
//! 1. `ProposalGraph` is the **canonical** representation of proposal dependencies; topological ordering is derived on demand.
//! 2. `KnowledgeEvolutionPlan` is a compiled **immutable** artifact.
//! 3. `PlanValidatorV2` is strictly **pure** and side-effect free.
//! 4. `EvolutionMutationSet` describes storage-agnostic **intent** only.
//! 5. Execution enforces an explicit state machine: `Pending` -> `Applied` -> `Committed` / `RolledBack`.

use crate::compiler::{EntityId, FactId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Strongly-typed identifier for an evolution plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlanId(pub Uuid);

impl std::fmt::Display for PlanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "plan_{}", self.0)
    }
}

/// Strongly-typed identifier for an evolution proposal item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProposalId(pub Uuid);

impl std::fmt::Display for ProposalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "prop_{}", self.0)
    }
}

/// Intent-based classification of evolution mutations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvolutionActionKind {
    /// Merge source entity into target entity.
    MergeEntities {
        /// Primary target entity ID.
        target_id: EntityId,
        /// Secondary source entity ID to be merged.
        source_id: EntityId,
    },
    /// Supersede a stale or contradictory fact.
    SupercedeFact {
        /// Target entity ID.
        target_entity_id: EntityId,
        /// Stale fact ID to be superseded.
        stale_fact_id: FactId,
    },
    /// Update confidence score for an entity.
    UpdateConfidence {
        /// Target entity ID.
        target_entity_id: EntityId,
        /// Recalibrated confidence score.
        new_confidence: f32,
    },
}

/// Individual suggested knowledge evolution mutation proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeEvolutionProposal {
    /// Unique proposal identifier.
    pub id: ProposalId,
    /// Storage-agnostic mutation action intent.
    pub action: EvolutionActionKind,
    /// Human-readable rationale or pass justification.
    pub reasoning: String,
    /// Confidence score [0.0..1.0].
    pub confidence: f32,
}

/// Directed dependency edge between two proposals (source must be executed before target).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProposalDependencyEdge {
    /// Prerequisite source proposal ID.
    pub source: ProposalId,
    /// Dependent target proposal ID.
    pub target: ProposalId,
}

/// Canonical dependency graph representation for an evolution plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProposalGraph {
    /// List of proposal nodes.
    pub nodes: Vec<KnowledgeEvolutionProposal>,
    /// Directed dependency edges.
    pub edges: Vec<ProposalDependencyEdge>,
}

impl ProposalGraph {
    /// Instantiates a new `ProposalGraph`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Derives topological execution ordering on demand. Returns an error if a cycle is detected.
    pub fn topological_sort(&self) -> Result<Vec<ProposalId>, String> {
        let mut in_degree: HashMap<ProposalId, usize> = HashMap::new();
        let mut adj: HashMap<ProposalId, Vec<ProposalId>> = HashMap::new();

        for node in &self.nodes {
            in_degree.insert(node.id, 0);
            adj.insert(node.id, Vec::new());
        }

        for edge in &self.edges {
            *in_degree.entry(edge.target).or_insert(0) += 1;
            adj.entry(edge.source).or_default().push(edge.target);
        }

        let mut queue: Vec<ProposalId> = in_degree
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
            Err("Cycle detected in ProposalGraph dependencies".to_string())
        } else {
            Ok(order)
        }
    }
}

/// Compiled immutable evolution plan artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeEvolutionPlan {
    /// Unique plan identifier.
    pub plan_id: PlanId,
    /// List of proposed evolution items.
    pub proposals: Vec<KnowledgeEvolutionProposal>,
    /// Canonical dependency graph.
    pub dependency_graph: ProposalGraph,
    /// Compilation timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Validation error item emitted by `PlanValidatorV2`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    /// Error category code.
    pub code: String,
    /// Detailed error explanation.
    pub message: String,
}

/// Pure validation result emitted by `PlanValidatorV2`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Flag indicating whether the plan is valid and safe for execution.
    pub is_valid: bool,
    /// List of validation errors discovered.
    pub errors: Vec<ValidationError>,
}

/// Execution state machine status for an evolution transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalExecutionState {
    /// Plan is queued for execution.
    Pending,
    /// Plan mutations applied transiently.
    Applied,
    /// Plan transaction successfully committed.
    Committed,
    /// Plan transaction rolled back due to error.
    RolledBack,
}

/// Intent-based storage-agnostic mutation set produced by `EvolutionExecutorV2`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EvolutionMutationSet {
    /// Entity merge intents (target, source).
    pub entity_merges: Vec<(EntityId, EntityId)>,
    /// Fact supersession intents (entity_id, stale_fact_id).
    pub fact_supercessions: Vec<(EntityId, FactId)>,
    /// Confidence update intents (entity_id, new_confidence).
    pub confidence_updates: Vec<(EntityId, f32)>,
}

/// First-class audit report documenting transactional execution outcomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionExecutionReport {
    /// Unique report execution UUID.
    pub report_id: Uuid,
    /// Target plan identifier.
    pub plan_id: PlanId,
    /// Final execution state machine status.
    pub final_state: ProposalExecutionState,
    /// Applied proposal IDs in order.
    pub applied_proposals: Vec<ProposalId>,
    /// Skipped proposal IDs.
    pub skipped_proposals: Vec<ProposalId>,
    /// Failed proposal IDs.
    pub failed_proposals: Vec<ProposalId>,
    /// Flag indicating if rollback occurred.
    pub rollback_occurred: bool,
    /// Execution duration in milliseconds.
    pub execution_duration_ms: u64,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
}
