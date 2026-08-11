//! Legacy evolution models maintained for backwards-compatibility.

use crate::artifact::EvidenceArtifactId;
use crate::execution::ExecutionId;
use crate::reasoning::PlanStepId;
use std::fmt;
use uuid::Uuid;

/// Strongly-typed identifier for a legacy evolution plan.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct EvolutionPlanId(pub Uuid);

impl EvolutionPlanId {
    /// Instantiates a new unique `EvolutionPlanId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for EvolutionPlanId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EvolutionPlanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "evo-plan-{}", self.0.simple())
    }
}

/// Strongly-typed identifier for a legacy evolution action.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct EvolutionActionId(pub Uuid);

impl EvolutionActionId {
    /// Instantiates a new unique `EvolutionActionId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for EvolutionActionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EvolutionActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "act-{}", self.0.simple())
    }
}

/// Strongly-typed identifier for a domain entity.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct DomainEntityId(pub Uuid);

impl DomainEntityId {
    /// Instantiates a new unique `DomainEntityId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for DomainEntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DomainEntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "entity-{}", self.0.simple())
    }
}

/// Legacy capability-oriented action proposed to evolve system memory or knowledge graph state.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LegacyEvolutionAction {
    /// Reinforce or strengthen relationship between source and target domain entities.
    ReinforceRelationship {
        /// Unique action identifier.
        id: EvolutionActionId,
        /// Source domain entity ID.
        source: DomainEntityId,
        /// Target domain entity ID.
        target: DomainEntityId,
    },
    /// Consolidate or merge redundant memory records.
    ConsolidateKnowledge {
        /// Unique action identifier.
        id: EvolutionActionId,
        /// Target memory entity ID.
        memory_id: DomainEntityId,
    },
    /// Mark conflicting or disputing artifact evidence.
    MarkConflict {
        /// Unique action identifier.
        id: EvolutionActionId,
        /// Conflicting artifact ID.
        artifact_id: EvidenceArtifactId,
    },
    /// Record an identified knowledge gap for future exploration.
    RecordKnowledgeGap {
        /// Unique action identifier.
        id: EvolutionActionId,
        /// Plan step ID where knowledge gap occurred.
        producer_step: PlanStepId,
    },
}

impl LegacyEvolutionAction {
    /// Returns the unique `EvolutionActionId` of this action.
    pub fn id(&self) -> EvolutionActionId {
        match self {
            Self::ReinforceRelationship { id, .. } => *id,
            Self::ConsolidateKnowledge { id, .. } => *id,
            Self::MarkConflict { id, .. } => *id,
            Self::RecordKnowledgeGap { id, .. } => *id,
        }
    }
}

/// Declarative evolution plan aggregate proposing a sequence of knowledge evolution actions.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LegacyEvolutionPlan {
    /// Unique evolution plan ID.
    pub id: EvolutionPlanId,
    /// Target execution run ID.
    pub execution_id: ExecutionId,
    /// Proposed list of evolution actions.
    pub actions: Vec<LegacyEvolutionAction>,
}

impl LegacyEvolutionPlan {
    /// Instantiates a new `LegacyEvolutionPlan`.
    pub fn new(execution_id: ExecutionId, actions: Vec<LegacyEvolutionAction>) -> Self {
        Self {
            id: EvolutionPlanId::new(),
            execution_id,
            actions,
        }
    }
}
