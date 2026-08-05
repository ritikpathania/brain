//! Domain models for Knowledge Graph Matching: GraphSimilarityScore, MatchRelationship, GraphMatch, GraphMatchSet, GraphMatchQuery, and GraphMatchReport.

use crate::candidate::{KnowledgeCandidate, KnowledgeCandidateId};
use crate::errors::DomainError;
use crate::evolution::DomainEntityId;
use crate::selection::EvidenceSet;
use std::collections::BTreeMap;
use std::fmt;

/// Opaque, invariant-checked numerical similarity score bounded between 0.0 and 1.0.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct GraphSimilarityScore(f32);

impl GraphSimilarityScore {
    /// Exact match constant (1.0).
    pub const EXACT: Self = Self(1.0);
    /// High similarity constant (0.8).
    pub const HIGH: Self = Self(0.8);
    /// Medium similarity constant (0.5).
    pub const MEDIUM: Self = Self(0.5);
    /// Low similarity constant (0.2).
    pub const LOW: Self = Self(0.2);
    /// Zero similarity constant (0.0).
    pub const NONE: Self = Self(0.0);

    /// Instantiates a new validated `GraphSimilarityScore`.
    pub fn new(val: f32) -> Result<Self, DomainError> {
        if !(0.0..=1.0).contains(&val) {
            return Err(DomainError::ValidationError {
                message: format!("Similarity score must be between 0.0 and 1.0, got {}", val),
                rule_id: Some("VAL-SIM-001".to_string()),
            });
        }
        Ok(Self(val))
    }

    /// Returns raw numerical score float value.
    pub fn value(&self) -> f32 {
        self.0
    }
}

impl Eq for GraphSimilarityScore {}

#[allow(clippy::derive_ord_xor_partial_ord)]
impl Ord for GraphSimilarityScore {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl Default for GraphSimilarityScore {
    fn default() -> Self {
        Self::NONE
    }
}

impl fmt::Display for GraphSimilarityScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

/// Semantic relationship classification between a candidate and an existing domain entity.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum MatchRelationship {
    /// Identical or duplicate entity.
    Duplicate,
    /// Partial concept or property overlap.
    Overlap,
    /// Logical or factual contradiction.
    Contradiction,
    /// Related or connected concept.
    Related,
}

impl fmt::Display for MatchRelationship {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate => write!(f, "Duplicate"),
            Self::Overlap => write!(f, "Overlap"),
            Self::Contradiction => write!(f, "Contradiction"),
            Self::Related => write!(f, "Related"),
        }
    }
}

/// Query encapsulation passing candidate and search parameters to matcher implementations.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GraphMatchQuery {
    /// Candidate payload to match against graph.
    pub candidate: KnowledgeCandidate,
    /// Optional limit on returned match count.
    pub limit: Option<usize>,
    /// Optional minimum similarity threshold score.
    pub minimum_similarity: Option<GraphSimilarityScore>,
}

impl GraphMatchQuery {
    /// Instantiates a new `GraphMatchQuery`.
    pub fn new(candidate: KnowledgeCandidate) -> Self {
        Self {
            candidate,
            limit: None,
            minimum_similarity: None,
        }
    }

    /// Sets limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets minimum similarity threshold score.
    pub fn with_minimum_similarity(mut self, min: GraphSimilarityScore) -> Self {
        self.minimum_similarity = Some(min);
        self
    }
}

/// Individual graph match item associating an existing entity with similarity, relationship, and evidence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GraphMatch {
    /// Existing domain entity ID.
    pub entity: DomainEntityId,
    /// Calculated similarity score.
    pub similarity: GraphSimilarityScore,
    /// Classified relationship type.
    pub relationship: MatchRelationship,
    /// Supporting evidence justifying match.
    pub matching_evidence: EvidenceSet,
}

impl GraphMatch {
    /// Instantiates a new `GraphMatch`.
    pub fn new(
        entity: DomainEntityId,
        similarity: GraphSimilarityScore,
        relationship: MatchRelationship,
        matching_evidence: EvidenceSet,
    ) -> Self {
        Self {
            entity,
            similarity,
            relationship,
            matching_evidence,
        }
    }
}

/// Opaque, deterministically ordered collection of graph matches for an entity.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GraphMatchSet {
    matches: BTreeMap<DomainEntityId, GraphMatch>,
}

impl GraphMatchSet {
    /// Instantiates a new empty `GraphMatchSet`.
    pub fn new() -> Self {
        Self {
            matches: BTreeMap::new(),
        }
    }

    /// Inserts a match into the set.
    pub fn insert(&mut self, match_item: GraphMatch) {
        self.matches.insert(match_item.entity, match_item);
    }

    /// Returns iterator over matches.
    pub fn iter(&self) -> impl Iterator<Item = &GraphMatch> {
        self.matches.values()
    }

    /// Returns match by entity ID if present.
    pub fn get(&self, entity: &DomainEntityId) -> Option<&GraphMatch> {
        self.matches.get(entity)
    }

    /// Returns highest similarity match if any.
    pub fn best_match(&self) -> Option<&GraphMatch> {
        self.matches.values().max_by_key(|m| m.similarity)
    }

    /// Returns true if entity is present in match set.
    pub fn contains(&self, entity: &DomainEntityId) -> bool {
        self.matches.contains_key(entity)
    }

    /// Returns number of matches in set.
    pub fn len(&self) -> usize {
        self.matches.len()
    }

    /// Returns true if set is empty.
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }
}

/// Opaque, immutable report containing derived observational graph matches for a KnowledgeCandidate.
/// Invariants:
/// - Opaque storage via `GraphMatchSet`.
/// - Observational only; contains zero confidence scores or consolidation decisions.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GraphMatchReport {
    /// Target candidate ID.
    pub candidate_id: KnowledgeCandidateId,
    /// Match set containing graph matches.
    pub match_set: GraphMatchSet,
}

impl GraphMatchReport {
    /// Instantiates a new `GraphMatchReport`.
    pub fn new(candidate_id: KnowledgeCandidateId, match_set: GraphMatchSet) -> Self {
        Self {
            candidate_id,
            match_set,
        }
    }

    /// Returns iterator over matches.
    pub fn iter(&self) -> impl Iterator<Item = &GraphMatch> {
        self.match_set.iter()
    }

    /// Returns highest similarity match if any.
    pub fn best_match(&self) -> Option<&GraphMatch> {
        self.match_set.best_match()
    }

    /// Returns true if entity is present in report match set.
    pub fn contains(&self, entity: &DomainEntityId) -> bool {
        self.match_set.contains(entity)
    }

    /// Returns number of matches in report.
    pub fn len(&self) -> usize {
        self.match_set.len()
    }

    /// Returns true if match set is empty.
    pub fn is_empty(&self) -> bool {
        self.match_set.is_empty()
    }
}
