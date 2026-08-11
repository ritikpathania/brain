//! Evolution proposals capturing planned graph evolutions.

use super::action::EvolutionAction;
use super::diff::SemanticDiff;
use crate::reflection::FindingId;
use crate::retrieval::ConfidenceAssessment;
use serde::{Deserialize, Serialize};

/// Priority tier assigned to an evolution proposal.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum Priority {
    /// Urgent structural conflict or major corruption risk.
    Critical,
    /// High-impact duplicate consolidation or entity promotion.
    High,
    /// Standard stewardship evolution.
    #[default]
    Medium,
    /// Minor cosmetic edge cleanup.
    Low,
}

/// Lifecycle status of an evolution proposal.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum ProposalStatus {
    /// Initial draft state during planning.
    Draft,
    /// Awaiting user review in UI.
    #[default]
    PendingReview,
    /// Approved by user, ready for execution.
    Approved,
    /// Explicitly rejected by user.
    Rejected,
    /// Successfully executed into graph store.
    Executed,
    /// Execution was undone/rolled back.
    RolledBack,
    /// Proposal expired due to upstream graph mutation.
    Expired,
}

/// Opaque newtype identifier for a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProposalId(pub uuid::Uuid);

impl Default for ProposalId {
    fn default() -> Self {
        Self::new()
    }
}

impl ProposalId {
    /// Generates a new random ProposalId.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for ProposalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "prop-{}", self.0)
    }
}

/// Provenance metadata tracking originating stewardship findings.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProposalOrigin {
    /// Originating stewardship finding identifiers.
    pub stewardship_findings: Vec<FindingId>,
}

/// Domain aggregate representing a planned knowledge evolution proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionProposal {
    /// Unique proposal identifier.
    pub id: ProposalId,
    /// Proposal provenance tracking originating findings.
    pub origin: ProposalOrigin,
    /// Human-readable title summarizing the proposal.
    pub title: String,
    /// Priority classification tier.
    pub priority: Priority,
    /// Confidence assessment score.
    pub confidence: ConfidenceAssessment,
    /// Graph mutation actions to execute.
    pub actions: Vec<EvolutionAction>,
    /// Human-readable semantic graph diff.
    pub diff: SemanticDiff,
    /// Current proposal lifecycle status.
    pub status: ProposalStatus,
}

impl EvolutionProposal {
    /// Creates a new EvolutionProposal.
    pub fn new(
        origin: ProposalOrigin,
        title: impl Into<String>,
        priority: Priority,
        confidence: ConfidenceAssessment,
        actions: Vec<EvolutionAction>,
        diff: SemanticDiff,
    ) -> Self {
        Self {
            id: ProposalId::new(),
            origin,
            title: title.into(),
            priority,
            confidence,
            actions,
            diff,
            status: ProposalStatus::PendingReview,
        }
    }

    /// Approves the proposal.
    pub fn approve(&mut self) {
        if self.status == ProposalStatus::PendingReview {
            self.status = ProposalStatus::Approved;
        }
    }

    /// Rejects the proposal.
    pub fn reject(&mut self) {
        if self.status == ProposalStatus::PendingReview {
            self.status = ProposalStatus::Rejected;
        }
    }

    /// Marks the proposal as expired.
    pub fn expire(&mut self) {
        self.status = ProposalStatus::Expired;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proposal_lifecycle_state_transitions() {
        let mut prop = EvolutionProposal::new(
            ProposalOrigin::default(),
            "Consolidate SQLite Notes",
            Priority::High,
            ConfidenceAssessment::new(0.95),
            vec![],
            SemanticDiff::default(),
        );

        assert_eq!(prop.status, ProposalStatus::PendingReview);
        prop.approve();
        assert_eq!(prop.status, ProposalStatus::Approved);
    }
}
