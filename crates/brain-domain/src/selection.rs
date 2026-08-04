//! Algebraic SelectionStrategy, SelectionContext, EvidenceQuery, and opaque EvidenceSet domain models.

use crate::artifact::{EvidenceArtifactId, EvidenceArtifactKind};
use crate::execution::ExecutionId;
use crate::reasoning::PlanStepId;
use std::collections::BTreeSet;

/// Composable algebraic strategy for selecting evidence artifacts from an ArtifactStore graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum SelectionStrategy {
    /// Select all artifacts in store.
    #[default]
    All,
    /// Select direct prerequisite artifacts produced by specified step ID.
    PrerequisitesOf(PlanStepId),
    /// Select artifacts matching specified representation kind.
    ByKind(EvidenceArtifactKind),
    /// Select all transitive upstream ancestor artifacts of target artifact ID.
    AncestorsOf(EvidenceArtifactId),
    /// Algebraic set union of two selection strategies.
    Union(Box<SelectionStrategy>, Box<SelectionStrategy>),
    /// Algebraic set intersection of two selection strategies.
    Intersection(Box<SelectionStrategy>, Box<SelectionStrategy>),
    /// Algebraic set difference of two selection strategies (Left - Right).
    Difference(Box<SelectionStrategy>, Box<SelectionStrategy>),
}

impl SelectionStrategy {
    /// Normalizes algebraic strategy trees into simplified canonical forms.
    pub fn normalize(self) -> Self {
        match self {
            Self::Union(left, right) => {
                let norm_left = left.normalize();
                let norm_right = right.normalize();
                if norm_left == norm_right {
                    norm_left
                } else if norm_left == Self::All {
                    Self::All
                } else if norm_right == Self::All {
                    Self::All
                } else {
                    Self::Union(Box::new(norm_left), Box::new(norm_right))
                }
            }
            Self::Intersection(left, right) => {
                let norm_left = left.normalize();
                let norm_right = right.normalize();
                if norm_left == norm_right {
                    norm_left
                } else if norm_left == Self::All {
                    norm_right
                } else if norm_right == Self::All {
                    norm_left
                } else {
                    Self::Intersection(Box::new(norm_left), Box::new(norm_right))
                }
            }
            Self::Difference(left, right) => {
                let norm_left = left.normalize();
                let norm_right = right.normalize();
                if norm_left == norm_right {
                    // Empty difference: intersect with impossible condition or return empty difference
                    Self::Difference(Box::new(norm_left.clone()), Box::new(norm_left))
                } else {
                    Self::Difference(Box::new(norm_left), Box::new(norm_right))
                }
            }
            other => other,
        }
    }
}

/// Immutable execution environment context passed during evidence selection.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelectionContext {
    /// Execution run ID.
    pub execution_id: ExecutionId,
}

impl SelectionContext {
    /// Instantiates a new `SelectionContext`.
    pub fn new(execution_id: ExecutionId) -> Self {
        Self { execution_id }
    }
}

/// Encapsulated query parameter combining selection strategy and execution context.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceQuery {
    /// Selection strategy tree.
    pub strategy: SelectionStrategy,
    /// Selection context environment.
    pub context: SelectionContext,
}

impl EvidenceQuery {
    /// Instantiates a new `EvidenceQuery`.
    pub fn new(strategy: SelectionStrategy, context: SelectionContext) -> Self {
        Self {
            strategy: strategy.normalize(),
            context,
        }
    }
}

/// Opaque, first-class evidence set holding a deduplicated, deterministically-ordered BTreeSet of artifact IDs.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct EvidenceSet {
    artifacts: BTreeSet<EvidenceArtifactId>,
    strategy: SelectionStrategy,
}

impl EvidenceSet {
    /// Instantiates a new `EvidenceSet`.
    pub fn new(artifacts: BTreeSet<EvidenceArtifactId>, strategy: SelectionStrategy) -> Self {
        Self {
            artifacts,
            strategy,
        }
    }

    /// Returns an iterator over the selected artifact IDs.
    pub fn iter(&self) -> impl Iterator<Item = &EvidenceArtifactId> {
        self.artifacts.iter()
    }

    /// Checks if target artifact ID is present in the evidence set.
    pub fn contains(&self, id: &EvidenceArtifactId) -> bool {
        self.artifacts.contains(id)
    }

    /// Returns the number of artifact IDs in the evidence set.
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    /// Returns whether the evidence set is empty.
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    /// Returns a reference to the selection strategy used.
    pub fn strategy(&self) -> &SelectionStrategy {
        &self.strategy
    }
}
