//! Append-only ArtifactStore maintaining evidence artifacts and directed provenance graph edges.

use crate::artifact::{
    ArtifactView, EvidenceArtifactId, ExecutionArtifact, ProvenanceEdge, ProvenanceEdgeId,
};
use crate::errors::DomainError;
use crate::reasoning::PlanStepId;
use std::collections::{HashMap, HashSet, VecDeque};

/// Append-only store managing evidence artifacts and provenance graph relationships.
///
/// Invariants:
/// - Artifact graph is append-only.
/// - Every provenance edge references existing artifacts in the store.
/// - Self-loops (`from == to`) and duplicate edges are strictly rejected.
/// - Parent ordering is deterministic.
#[derive(Debug, Clone, Default)]
pub struct ArtifactStore {
    artifacts: HashMap<EvidenceArtifactId, ExecutionArtifact>,
    step_to_artifact: HashMap<PlanStepId, EvidenceArtifactId>,
    edges: HashMap<ProvenanceEdgeId, ProvenanceEdge>,
    // Adjacency maps for fast query resolution
    outgoing: HashMap<EvidenceArtifactId, Vec<ProvenanceEdgeId>>,
    incoming: HashMap<EvidenceArtifactId, Vec<ProvenanceEdgeId>>,
}

impl ArtifactStore {
    /// Instantiates a new, empty `ArtifactStore`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts an `ExecutionArtifact` into the store.
    /// Invariant: Artifacts are immutable and append-only once inserted.
    pub fn insert(
        &mut self,
        artifact: ExecutionArtifact,
    ) -> Result<EvidenceArtifactId, DomainError> {
        let id = artifact.id;
        let step_id = artifact.metadata.producer_step;

        if self.artifacts.contains_key(&id) {
            return Err(DomainError::ValidationError {
                message: format!("Artifact ID {} already exists in store", id),
                rule_id: Some("VAL-ART-001".to_string()),
            });
        }

        self.step_to_artifact.insert(step_id, id);
        self.artifacts.insert(id, artifact);
        Ok(id)
    }

    /// Appends a new `ProvenanceEdge` connecting a source artifact to a target artifact.
    ///
    /// Invariants enforced:
    /// 1. Both `edge.from` and `edge.to` must exist in the store.
    /// 2. Self-loops (`from == to`) are rejected.
    /// 3. Duplicate identical edges (`from`, `to`, `relationship`) are rejected.
    pub fn add_edge(&mut self, edge: ProvenanceEdge) -> Result<ProvenanceEdgeId, DomainError> {
        if edge.from == edge.to {
            return Err(DomainError::ValidationError {
                message: format!(
                    "Self-loop provenance edge is invalid for artifact {}",
                    edge.from
                ),
                rule_id: Some("VAL-ART-002".to_string()),
            });
        }

        if !self.artifacts.contains_key(&edge.from) {
            return Err(DomainError::ValidationError {
                message: format!(
                    "Source artifact {} does not exist in ArtifactStore",
                    edge.from
                ),
                rule_id: Some("VAL-ART-003".to_string()),
            });
        }

        if !self.artifacts.contains_key(&edge.to) {
            return Err(DomainError::ValidationError {
                message: format!(
                    "Target artifact {} does not exist in ArtifactStore",
                    edge.to
                ),
                rule_id: Some("VAL-ART-004".to_string()),
            });
        }

        // Check duplicate edge invariant
        if let Some(existing_outgoing) = self.outgoing.get(&edge.from) {
            for existing_id in existing_outgoing {
                if let Some(existing_edge) = self.edges.get(existing_id) {
                    if existing_edge.to == edge.to
                        && existing_edge.relationship == edge.relationship
                    {
                        return Err(DomainError::ValidationError {
                            message: format!(
                                "Duplicate provenance edge {:?} between {} -> {}",
                                edge.relationship, edge.from, edge.to
                            ),
                            rule_id: Some("VAL-ART-005".to_string()),
                        });
                    }
                }
            }
        }

        let edge_id = edge.id;
        self.outgoing.entry(edge.from).or_default().push(edge_id);
        self.incoming.entry(edge.to).or_default().push(edge_id);
        self.edges.insert(edge_id, edge);

        Ok(edge_id)
    }

    /// Looks up an artifact by ID, returning an immutable `ArtifactView`.
    pub fn get(&self, id: EvidenceArtifactId) -> Option<ArtifactView<'_>> {
        self.artifacts.get(&id).map(ArtifactView::new)
    }

    /// Looks up an artifact produced by a specific plan step ID.
    pub fn get_by_producer(&self, step_id: PlanStepId) -> Option<ArtifactView<'_>> {
        self.step_to_artifact
            .get(&step_id)
            .and_then(|id| self.get(*id))
    }

    /// Returns a list of all artifact IDs present in the store in deterministic sorted order.
    pub fn all_artifact_ids(&self) -> Vec<EvidenceArtifactId> {
        let mut ids: Vec<_> = self.artifacts.keys().copied().collect();
        ids.sort();
        ids
    }

    /// Returns an iterator over all artifact views present in the store.
    pub fn all_artifact_views(&self) -> impl Iterator<Item = ArtifactView<'_>> {
        self.artifacts.values().map(ArtifactView::new)
    }

    /// Returns direct parent artifact IDs for a target child artifact ID in deterministic order.
    pub fn parents(&self, id: EvidenceArtifactId) -> Vec<EvidenceArtifactId> {
        let mut parent_ids = Vec::new();
        if let Some(incoming_edges) = self.incoming.get(&id) {
            for edge_id in incoming_edges {
                if let Some(edge) = self.edges.get(edge_id) {
                    parent_ids.push(edge.from);
                }
            }
        }
        parent_ids.sort();
        parent_ids.dedup();
        parent_ids
    }

    /// Returns direct child artifact IDs for a source parent artifact ID in deterministic order.
    pub fn children(&self, id: EvidenceArtifactId) -> Vec<EvidenceArtifactId> {
        let mut child_ids = Vec::new();
        if let Some(outgoing_edges) = self.outgoing.get(&id) {
            for edge_id in outgoing_edges {
                if let Some(edge) = self.edges.get(edge_id) {
                    child_ids.push(edge.to);
                }
            }
        }
        child_ids.sort();
        child_ids.dedup();
        child_ids
    }

    /// Traverses all upstream ancestor artifact IDs transitively in deterministic topological order without duplicate visits.
    pub fn ancestors(&self, id: EvidenceArtifactId) -> Vec<EvidenceArtifactId> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(id);
        visited.insert(id);

        while let Some(curr) = queue.pop_front() {
            let parents = self.parents(curr);
            for parent_id in parents {
                if visited.insert(parent_id) {
                    result.push(parent_id);
                    queue.push_back(parent_id);
                }
            }
        }

        result
    }

    /// Traverses all downstream descendant artifact IDs transitively in deterministic topological order without duplicate visits.
    pub fn descendants(&self, id: EvidenceArtifactId) -> Vec<EvidenceArtifactId> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(id);
        visited.insert(id);

        while let Some(curr) = queue.pop_front() {
            let children = self.children(curr);
            for child_id in children {
                if visited.insert(child_id) {
                    result.push(child_id);
                    queue.push_back(child_id);
                }
            }
        }

        result
    }
}
