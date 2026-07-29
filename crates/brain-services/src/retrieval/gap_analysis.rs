//! Deterministic Knowledge Gap Analysis engine and structured reporting.

use crate::reconciliation::ContradictionRecord;
use crate::retrieval::contracts::{EvidenceSet, QueryContext};
use serde::{Deserialize, Serialize};

/// Structured knowledge gap analysis report detailing known facts vs missing/weak knowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct KnowledgeGapReport {
    /// Target query context string.
    pub query: String,
    /// Direct matching evidence and known entity facts.
    pub known_facts: Vec<String>,
    /// Unfilled attributes or missing relationships on target entities.
    pub unknown_attributes: Vec<String>,
    /// Facts backed by low confidence or weak retention evidence.
    pub weak_evidence: Vec<String>,
    /// Conflicting evidence or contradiction records detected.
    pub conflicting_evidence: Vec<String>,
    /// Suggested observations or ingest steps to resolve gaps.
    pub suggested_observations: Vec<String>,
}

/// Analyzer constructing deterministic `KnowledgeGapReport`s from evidence sets and contradictions.
#[derive(Debug, Clone, Default)]
pub struct GapAnalyzer;

impl GapAnalyzer {
    /// Creates a new `GapAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates an `EvidenceSet` and optional contradiction records to produce a deterministic `KnowledgeGapReport`.
    pub fn analyze(
        &self,
        query: &QueryContext,
        evidence: &EvidenceSet,
        contradictions: &[ContradictionRecord],
    ) -> KnowledgeGapReport {
        let mut known_facts = Vec::new();
        let mut unknown_attributes = Vec::new();
        let mut weak_evidence = Vec::new();
        let mut conflicting_evidence = Vec::new();
        let mut suggested_observations = Vec::new();

        if evidence.items.is_empty() {
            unknown_attributes.push(format!(
                "No canonical entities found matching query '{}'",
                query.query_string
            ));
            suggested_observations.push(format!(
                "Ingest raw documents or notes regarding '{}'",
                query.query_string
            ));
        } else {
            for item in &evidence.items {
                if item.final_score >= 0.7 {
                    known_facts.push(format!(
                        "Entity '{}' [id={}]",
                        item.preferred_name, item.entity_id
                    ));
                } else {
                    weak_evidence.push(format!(
                        "Entity '{}' has low retrieval confidence ({:.2})",
                        item.preferred_name, item.final_score
                    ));
                    suggested_observations.push(format!(
                        "Provide additional observations for '{}'",
                        item.preferred_name
                    ));
                }
            }
        }

        for record in contradictions {
            conflicting_evidence.push(format!(
                "Contradiction [{:?}]: {}",
                record.kind, record.rationale
            ));
            suggested_observations.push(format!(
                "Reconcile contradictory entity pair ({} vs {})",
                record.entity_a, record.entity_b
            ));
        }

        KnowledgeGapReport {
            query: query.query_string.clone(),
            known_facts,
            unknown_attributes,
            weak_evidence,
            conflicting_evidence,
            suggested_observations,
        }
    }
}
