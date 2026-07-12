use crate::identifiers::RelationId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The directionality of a relationship edge (directed or undirected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Directionality {
    /// The relationship has a source and target where orientation matters.
    Directed,
    /// The relationship represents a bidirectional/unoriented association.
    Undirected,
}

/// Strategies to aggregate confidence weights/scores for multiple instances of a relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceStrategy {
    /// Take the maximum of all recorded scores.
    Maximum,
    /// Take the average of all recorded scores.
    Average,
    /// Take the minimum of all recorded scores.
    Minimum,
    /// Interpret the score strictly from the extraction score.
    SourceDefined,
}

impl ConfidenceStrategy {
    /// Combines two confidence scores/weights using this strategy.
    pub fn combine(&self, w1: f64, w2: f64) -> f64 {
        match self {
            ConfidenceStrategy::Maximum => f64::max(w1, w2),
            ConfidenceStrategy::Average => (w1 + w2) / 2.0,
            ConfidenceStrategy::Minimum => f64::min(w1, w2),
            ConfidenceStrategy::SourceDefined => w1 * w2,
        }
    }
}

/// Schema representing metadata and rules for a registered graph relation type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationDefinition {
    /// Stable relation newtype identifier (e.g. "depends_on", "associated_with").
    pub id: RelationId,
    /// Human-readable label for rendering and interfaces.
    pub display_name: String,
    /// The symmetric/reverse relation identifier, if defined.
    pub inverse: Option<RelationId>,
    /// The directionality (directed or undirected) of the edge.
    pub directionality: Directionality,
    /// Indicates whether the relation is symmetric (A -> B implies B -> A).
    pub symmetry: bool,
    /// Indicates whether the relation is transitive (A -> B and B -> C implies A -> C).
    pub transitivity: bool,
    /// Indicates whether this specific relation suppresses generic fallback association edges.
    pub fallback_suppression: bool,
    /// The method used to interpret and aggregate relation weights.
    pub confidence_strategy: ConfidenceStrategy,
    /// A description of the relation's semantic meaning and intended usage.
    pub description: String,
}

/// Errors returned by the relation registry during loading or invariant checking.
#[derive(Debug, thiserror::Error)]
pub enum RelationRegistryError {
    /// Error returned when duplicate relation IDs are detected.
    #[error("Duplicate relation ID found in registry: {0}")]
    DuplicateRelation(RelationId),

    /// Error returned when a relation definition lacks an identifier.
    #[error("Relation ID cannot be empty")]
    EmptyId,

    /// Error returned when a relation definition lacks a display name.
    #[error("Relation display name cannot be empty for ID: {0}")]
    EmptyDisplayName(RelationId),

    /// Error returned when a relation references an inverse ID that is not defined in the registry.
    #[error("Relation '{relation}' specifies inverse '{inverse}' which does not exist in the registry")]
    MissingInverse {
        /// The referencing relation ID.
        relation: RelationId,
        /// The declared missing inverse relation ID.
        inverse: RelationId,
    },

    /// Error returned when an inverse pair does not map symmetrically (A -> B but B -> C).
    #[error("Asymmetric inverse relation mismatch: '{rel_a}' specifies '{rel_b}' as inverse, but '{rel_b}' specifies '{inverse_b:?}' instead")]
    InvalidInversePair {
        /// The first relation ID.
        rel_a: RelationId,
        /// The second relation ID.
        rel_b: RelationId,
        /// The actual inverse ID defined by the second relation.
        inverse_b: Option<RelationId>,
    },

    /// Error returned when an undirected relation is not marked as symmetric.
    #[error("Undirected relation '{0}' must be marked as symmetric")]
    UndirectedNotSymmetric(RelationId),

    /// Error returned when a symmetric relation specifies a distinct relation ID as its inverse.
    #[error("Symmetric relation '{relation}' cannot declare a distinct relation '{inverse}' as its inverse")]
    SymmetricHasDistinctInverse {
        /// The symmetric relation ID.
        relation: RelationId,
        /// The distinct invalid inverse relation ID.
        inverse: RelationId,
    },
}

/// Encapsulated, read-only declarative relation registry.
///
/// Ensures all loaded relations satisfy semantic invariants on construction.
pub struct RelationRegistry {
    definitions: HashMap<RelationId, RelationDefinition>,
}

impl RelationRegistry {
    /// Creates and validates a new RelationRegistry from the supplied list of definitions.
    ///
    /// Validates duplicate IDs, empty fields, and ontology consistency.
    pub fn new(defs: Vec<RelationDefinition>) -> Result<Self, RelationRegistryError> {
        let mut definitions = HashMap::with_capacity(defs.len());

        for def in defs {
            if def.id.as_str().trim().is_empty() {
                return Err(RelationRegistryError::EmptyId);
            }
            if def.display_name.trim().is_empty() {
                return Err(RelationRegistryError::EmptyDisplayName(def.id));
            }
            if definitions.contains_key(&def.id) {
                return Err(RelationRegistryError::DuplicateRelation(def.id));
            }
            definitions.insert(def.id.clone(), def);
        }

        // Validate ontology invariants
        for def in definitions.values() {
            // Undirected implies symmetric
            if def.directionality == Directionality::Undirected && !def.symmetry {
                return Err(RelationRegistryError::UndirectedNotSymmetric(def.id.clone()));
            }

            if let Some(ref inv) = def.inverse {
                // Inverse must exist in the registry
                let inv_def = definitions.get(inv).ok_or_else(|| {
                    RelationRegistryError::MissingInverse {
                        relation: def.id.clone(),
                        inverse: inv.clone(),
                    }
                })?;

                // Symmetric relations can only have themselves as an inverse
                if def.symmetry && inv != &def.id {
                    return Err(RelationRegistryError::SymmetricHasDistinctInverse {
                        relation: def.id.clone(),
                        inverse: inv.clone(),
                    });
                }

                // Verify inverse symmetry: inv_def.inverse must be Some(def.id)
                match &inv_def.inverse {
                    Some(inv_inv) if inv_inv == &def.id => {}
                    other => {
                        return Err(RelationRegistryError::InvalidInversePair {
                            rel_a: def.id.clone(),
                            rel_b: inv.clone(),
                            inverse_b: other.clone(),
                        });
                    }
                }
            }
        }

        Ok(Self { definitions })
    }

    /// Retrieves a reference to the definition of a relation.
    ///
    /// Interoperates with string slices and RelationId via AsRef<str> and Borrow<str>.
    pub fn get<K: AsRef<str>>(&self, id: K) -> Option<&RelationDefinition> {
        self.definitions.get(id.as_ref())
    }

    /// Retrieves a reference to the relation definition using the typed RelationKind.
    pub fn get_kind(&self, kind: crate::entities::RelationKind) -> Option<&RelationDefinition> {
        self.get(kind.id())
    }

    /// Returns true if the registry contains the specified relation.
    ///
    /// Interoperates with string slices and RelationId via AsRef<str> and Borrow<str>.
    pub fn contains<K: AsRef<str>>(&self, id: K) -> bool {
        self.definitions.contains_key(id.as_ref())
    }

    /// Returns true if the registry contains the specified RelationKind.
    pub fn contains_kind(&self, kind: crate::entities::RelationKind) -> bool {
        self.contains(kind.id())
    }

    /// Returns an iterator over all relation definitions in the registry.
    pub fn iter(&self) -> impl Iterator<Item = &RelationDefinition> {
        self.definitions.values()
    }

    /// Returns the total number of registered relations.
    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    /// Loads a default RelationRegistry from the embedded relations.json file.
    pub fn default_embedded() -> Self {
        let json_str = include_str!("../../../protocol/relations.json");
        let defs: Vec<RelationDefinition> = serde_json::from_str(json_str)
            .expect("Failed to deserialize embedded relations.json");
        Self::new(defs).expect("Failed to construct default RelationRegistry")
    }

    /// Resolves the TraversalPolicy for a given relation.
    pub fn traversal_policy<K: AsRef<str>>(&self, relation: K) -> TraversalPolicy {
        if let Some(def) = self.get(relation) {
            TraversalPolicy {
                can_follow_forward: true,
                can_follow_backward: def.directionality == Directionality::Undirected || def.symmetry,
            }
        } else {
            TraversalPolicy {
                can_follow_forward: false,
                can_follow_backward: false,
            }
        }
    }
}

/// Resolved traversal policy for a given relation, encapsulating directionality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TraversalPolicy {
    /// Allowed to traverse in the forward direction (source -> target).
    pub can_follow_forward: bool,
    /// Allowed to traverse in the backward direction (target -> source).
    pub can_follow_backward: bool,
}

impl TraversalPolicy {
    /// Checks if traversal is allowed in the specified direction.
    pub fn can_traverse(&self, is_forward: bool) -> bool {
        if is_forward {
            self.can_follow_forward
        } else {
            self.can_follow_backward
        }
    }
}
