//! Immutable ExecutionArtifacts, first-class ProvenanceEdges, and artifact metadata representation.

use crate::execution::{ExecutionId, ExecutionTimestamp};
use crate::reasoning::PlanStepId;
use crate::value::StructuredValue;
use std::fmt;
use uuid::Uuid;

/// Strongly-typed identifier for an execution artifact.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct EvidenceArtifactId(pub Uuid);

impl EvidenceArtifactId {
    /// Instantiates a new unique `EvidenceArtifactId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for EvidenceArtifactId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EvidenceArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "art-{}", self.0.simple())
    }
}

/// Strongly-typed identifier for a provenance edge in the evidence graph.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ProvenanceEdgeId(pub Uuid);

impl ProvenanceEdgeId {
    /// Instantiates a new unique `ProvenanceEdgeId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ProvenanceEdgeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProvenanceEdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "edge-{}", self.0.simple())
    }
}

/// Taxonomy of artifact data representations.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum EvidenceArtifactKind {
    /// Unprocessed raw payload data from retrieval or tool calls.
    RawData,
    /// Intermediate processed or filtered data payload.
    DerivedData,
    /// Extracted factual claim or proposition.
    Claim,
    /// Condensed summary of multiple evidence items.
    Summary,
    /// Final synthesized output result.
    Result,
}

impl fmt::Display for EvidenceArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawData => write!(f, "Raw Data"),
            Self::DerivedData => write!(f, "Derived Data"),
            Self::Claim => write!(f, "Claim"),
            Self::Summary => write!(f, "Summary"),
            Self::Result => write!(f, "Result"),
        }
    }
}

/// Semantic provenance relationship linking a source artifact to a target artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ProvenanceRelationship {
    /// Target artifact was derived directly from source artifact.
    DerivedFrom,
    /// Target artifact references or cites source artifact.
    References,
    /// Target artifact condenses or summarizes source artifact.
    Summarizes,
    /// Target artifact contradicts or disputes source artifact.
    Contradicts,
}

impl fmt::Display for ProvenanceRelationship {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DerivedFrom => write!(f, "DerivedFrom"),
            Self::References => write!(f, "References"),
            Self::Summarizes => write!(f, "Summarizes"),
            Self::Contradicts => write!(f, "Contradicts"),
        }
    }
}

/// First-class entity representing a directed provenance relationship between artifacts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProvenanceEdge {
    /// Unique identifier for this provenance edge.
    pub id: ProvenanceEdgeId,
    /// Source artifact ID (parent/origin).
    pub from: EvidenceArtifactId,
    /// Target artifact ID (child/dependent).
    pub to: EvidenceArtifactId,
    /// Semantic relationship classification.
    pub relationship: ProvenanceRelationship,
    /// Edge creation timestamp.
    pub created_at: ExecutionTimestamp,
}

impl ProvenanceEdge {
    /// Instantiates a new `ProvenanceEdge`.
    pub fn new(
        from: EvidenceArtifactId,
        to: EvidenceArtifactId,
        relationship: ProvenanceRelationship,
        created_at: ExecutionTimestamp,
    ) -> Self {
        Self {
            id: ProvenanceEdgeId::new(),
            from,
            to,
            relationship,
            created_at,
        }
    }
}

/// Production metadata accompanying an execution artifact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArtifactMetadata {
    /// Representation classification.
    pub kind: EvidenceArtifactKind,
    /// Producing plan step ID.
    pub producer_step: PlanStepId,
    /// Execution run ID.
    pub execution_id: ExecutionId,
    /// Artifact creation timestamp.
    pub created_at: ExecutionTimestamp,
}

/// Immutable value object representing a generated evidence artifact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionArtifact {
    /// Unique artifact ID.
    pub id: EvidenceArtifactId,
    /// Production metadata.
    pub metadata: ArtifactMetadata,
    /// Canonical domain structured data.
    pub value: StructuredValue,
}

impl ExecutionArtifact {
    /// Instantiates a new `ExecutionArtifact`.
    pub fn new(metadata: ArtifactMetadata, value: StructuredValue) -> Self {
        Self {
            id: EvidenceArtifactId::new(),
            metadata,
            value,
        }
    }
}

/// Immutable read-only view wrapper over an `ExecutionArtifact`.
#[derive(Debug, Clone, Copy)]
pub struct ArtifactView<'a> {
    artifact: &'a ExecutionArtifact,
}

impl<'a> ArtifactView<'a> {
    /// Wraps an `ExecutionArtifact` reference in an `ArtifactView`.
    pub fn new(artifact: &'a ExecutionArtifact) -> Self {
        Self { artifact }
    }

    /// Accesses the artifact ID.
    pub fn id(&self) -> EvidenceArtifactId {
        self.artifact.id
    }

    /// Accesses the artifact metadata.
    pub fn metadata(&self) -> &ArtifactMetadata {
        &self.artifact.metadata
    }

    /// Accesses the artifact value.
    pub fn value(&self) -> &StructuredValue {
        &self.artifact.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_and_edge_construction() {
        let meta = ArtifactMetadata {
            kind: EvidenceArtifactKind::RawData,
            producer_step: PlanStepId::new(1),
            execution_id: ExecutionId::new(),
            created_at: ExecutionTimestamp::now(),
        };

        let artifact = ExecutionArtifact::new(meta, StructuredValue::String("data".to_string()));
        let view = ArtifactView::new(&artifact);

        assert_eq!(view.id(), artifact.id);
        assert_eq!(view.value(), &StructuredValue::String("data".to_string()));

        let edge = ProvenanceEdge::new(
            artifact.id,
            EvidenceArtifactId::new(),
            ProvenanceRelationship::DerivedFrom,
            ExecutionTimestamp::now(),
        );

        assert_eq!(edge.relationship, ProvenanceRelationship::DerivedFrom);
    }
}
