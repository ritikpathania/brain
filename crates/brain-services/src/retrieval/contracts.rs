//! Stable trait contracts and core data structures for the two-tier retrieval architecture.

use brain_domain::{EntityId, KnowledgeEvidence, SourceId};
use std::collections::HashMap;

/// Context parameters passed into the retrieval pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryContext {
    /// Target query search string or symbol key.
    pub query_string: String,
    /// Maximum candidate count to retrieve.
    pub limit: usize,
    /// Optional entity scope constraints.
    pub target_entities: Option<Vec<EntityId>>,
}

/// Discovered retrieval candidate before scoring and ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Candidate entity identifier.
    pub entity_id: EntityId,
    /// Preferred name or label of the candidate entity.
    pub preferred_name: String,
    /// Initial retrieval match metadata.
    pub initial_score: f32,
    /// Name of the retriever that discovered this candidate.
    pub retriever_source: &'static str,
}

/// Collection of candidates returned by a retriever.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CandidateSet {
    /// Discovered candidate records.
    pub candidates: Vec<Candidate>,
}

impl CandidateSet {
    /// Creates a new empty `CandidateSet`.
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
        }
    }

    /// Adds a candidate to the set.
    pub fn add(&mut self, candidate: Candidate) {
        if !self
            .candidates
            .iter()
            .any(|c| c.entity_id == candidate.entity_id)
        {
            self.candidates.push(candidate);
        }
    }
}

/// Multi-dimensional feature score vector returned by a scorer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScoreVector {
    /// Individual feature scores keyed by feature name (e.g. "fts", "graph_distance", "confidence").
    pub features: HashMap<String, f32>,
}

/// Assembled evidence item with source provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceItem {
    /// Target entity identifier.
    pub entity_id: EntityId,
    /// Preferred display name.
    pub preferred_name: String,
    /// Final normalized score.
    pub final_score: f32,
    /// Supporting evidence container.
    pub evidence: KnowledgeEvidence,
    /// Observation sources that contributed to this evidence item.
    pub sources: Vec<SourceId>,
}

/// Snapshot collection of ranked evidence returned by the retrieval engine.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EvidenceSet {
    /// Ranked evidence items in descending relevance order.
    pub items: Vec<EvidenceItem>,
}

/// Candidate discovery contract.
pub trait Retriever: Send + Sync {
    /// Returns the static name of the retriever.
    fn name(&self) -> &'static str;

    /// Discovers candidate entities matching the query context.
    fn retrieve(&self, query: &QueryContext) -> CandidateSet;
}

/// Feature scoring contract.
pub trait Scorer: Send + Sync {
    /// Returns the static name of the scorer.
    fn name(&self) -> &'static str;

    /// Computes multi-dimensional feature scores for a candidate.
    fn score(&self, candidate: &Candidate, query: &QueryContext) -> ScoreVector;
}

/// Ranking policy contract combining candidate sets and feature scores into a ranked EvidenceSet.
pub trait RankPolicy: Send + Sync {
    /// Ranks candidates using score vectors according to policy rules.
    fn rank(&self, candidates: CandidateSet, scores: Vec<ScoreVector>) -> EvidenceSet;
}
