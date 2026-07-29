//! Concrete policy traits and default implementations for knowledge lifecycle transitions.

use brain_domain::{KnowledgeEvidence, KnowledgeState, RetentionTier};

/// Policy governing lifecycle state transitions based on evidence freshness and observation reliability.
pub trait LifecyclePolicy: Send + Sync {
    /// Evaluates the next lifecycle state for a given entity given its current state and evidence container.
    fn evaluate_transition(
        &self,
        current: KnowledgeState,
        evidence: &KnowledgeEvidence,
    ) -> KnowledgeState;
}

/// Default lifecycle policy implementation evaluating observation confidence, tier, and source reliability.
#[derive(Debug, Clone)]
pub struct DefaultLifecyclePolicy {
    /// Reliability threshold to promote Observed -> Verified.
    pub verification_threshold: f32,
    /// Reliability threshold to demote Weak -> Deprecated.
    pub deprecation_threshold: f32,
}

impl Default for DefaultLifecyclePolicy {
    fn default() -> Self {
        Self {
            verification_threshold: 0.8,
            deprecation_threshold: 0.2,
        }
    }
}

impl LifecyclePolicy for DefaultLifecyclePolicy {
    fn evaluate_transition(
        &self,
        current: KnowledgeState,
        evidence: &KnowledgeEvidence,
    ) -> KnowledgeState {
        match current {
            KnowledgeState::Observed => {
                if evidence.source_reliability >= self.verification_threshold {
                    KnowledgeState::Verified
                } else if evidence.source_reliability < self.deprecation_threshold {
                    KnowledgeState::Weak
                } else {
                    KnowledgeState::Observed
                }
            }
            KnowledgeState::Verified => {
                if let RetentionTier::Recent(records) = &evidence.retention {
                    if records.len() >= 3 {
                        return KnowledgeState::Reinforced;
                    }
                }
                KnowledgeState::Verified
            }
            KnowledgeState::Reinforced => KnowledgeState::Reinforced,
            KnowledgeState::Weak => {
                if evidence.source_reliability < self.deprecation_threshold {
                    KnowledgeState::Deprecated
                } else {
                    KnowledgeState::Weak
                }
            }
            KnowledgeState::Deprecated => KnowledgeState::Deprecated,
            KnowledgeState::Archived => KnowledgeState::Archived,
        }
    }
}
