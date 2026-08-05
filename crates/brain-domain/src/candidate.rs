//! Domain models for Knowledge Candidates: CandidateConfidence, KnowledgeCandidateId, KnowledgeCandidate, and KnowledgeCandidateSet.

use crate::errors::DomainError;
use crate::execution::ExecutionId;
use crate::selection::EvidenceSet;
use crate::synthesis::ReasoningFindingId;
use crate::value::StructuredValue;
use std::collections::BTreeMap;
use std::fmt;
use uuid::Uuid;

/// Opaque, invariant-checked numerical confidence value object bounded between 0.0 and 1.0.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct CandidateConfidence(f32);

impl CandidateConfidence {
    /// Standard high confidence constant (0.9).
    pub const HIGH: Self = Self(0.9);
    /// Standard medium confidence constant (0.6).
    pub const MEDIUM: Self = Self(0.6);
    /// Standard low confidence constant (0.3).
    pub const LOW: Self = Self(0.3);
    /// Zero confidence constant (0.0).
    pub const ZERO: Self = Self(0.0);

    /// Instantiates a new validated `CandidateConfidence` value object.
    pub fn new(val: f32) -> Result<Self, DomainError> {
        if !(0.0..=1.0).contains(&val) {
            return Err(DomainError::ValidationError {
                message: format!("Confidence value must be between 0.0 and 1.0, got {}", val),
                rule_id: Some("VAL-CONF-001".to_string()),
            });
        }
        Ok(Self(val))
    }

    /// Returns the raw numerical float value.
    pub fn value(&self) -> f32 {
        self.0
    }
}

impl Eq for CandidateConfidence {}

#[allow(clippy::derive_ord_xor_partial_ord)]
impl Ord for CandidateConfidence {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl Default for CandidateConfidence {
    fn default() -> Self {
        Self::MEDIUM
    }
}

impl fmt::Display for CandidateConfidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

/// Strongly-typed identifier for a knowledge candidate.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct KnowledgeCandidateId(pub Uuid);

impl KnowledgeCandidateId {
    /// Instantiates a new unique `KnowledgeCandidateId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for KnowledgeCandidateId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for KnowledgeCandidateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cand-{}", self.0.simple())
    }
}

/// Immutable, self-contained domain object representing a potential piece of long-term knowledge.
/// Invariant: KnowledgeCandidate is not yet memory until an explicit consolidation decision is made.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeCandidate {
    /// Unique candidate identifier.
    pub id: KnowledgeCandidateId,
    /// Origin execution run ID for full provenance tracking.
    pub origin_execution: ExecutionId,
    /// Target reasoning finding ID from which candidate was extracted.
    pub finding_id: ReasoningFindingId,
    /// Supporting evidence set.
    pub evidence: EvidenceSet,
    /// Derived confidence value.
    pub confidence: CandidateConfidence,
    /// Structured domain payload value.
    pub payload: StructuredValue,
}

impl KnowledgeCandidate {
    /// Instantiates a new `KnowledgeCandidate`.
    pub fn new(
        origin_execution: ExecutionId,
        finding_id: ReasoningFindingId,
        evidence: EvidenceSet,
        confidence: CandidateConfidence,
        payload: StructuredValue,
    ) -> Self {
        Self {
            id: KnowledgeCandidateId::new(),
            origin_execution,
            finding_id,
            evidence,
            confidence,
            payload,
        }
    }
}

/// Opaque, deterministically ordered collection of knowledge candidates.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeCandidateSet {
    candidates: BTreeMap<KnowledgeCandidateId, KnowledgeCandidate>,
}

impl KnowledgeCandidateSet {
    /// Instantiates a new empty `KnowledgeCandidateSet`.
    pub fn new() -> Self {
        Self {
            candidates: BTreeMap::new(),
        }
    }

    /// Inserts a candidate into the set.
    pub fn insert(&mut self, candidate: KnowledgeCandidate) {
        self.candidates.insert(candidate.id, candidate);
    }

    /// Returns an iterator over candidates in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = &KnowledgeCandidate> {
        self.candidates.values()
    }

    /// Returns candidate by ID if present.
    pub fn get(&self, id: &KnowledgeCandidateId) -> Option<&KnowledgeCandidate> {
        self.candidates.get(id)
    }

    /// Returns the number of candidates in the set.
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Returns true if the candidate set is empty.
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

impl FromIterator<KnowledgeCandidate> for KnowledgeCandidateSet {
    fn from_iter<T: IntoIterator<Item = KnowledgeCandidate>>(iter: T) -> Self {
        let mut set = Self::new();
        for candidate in iter {
            set.insert(candidate);
        }
        set
    }
}
