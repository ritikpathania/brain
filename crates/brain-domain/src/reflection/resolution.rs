//! Action resolutions applied to stewardship findings.

use super::finding::FindingId;
use super::recommendation::RecommendationId;
use serde::{Deserialize, Serialize};

/// Policy governing automatic resolution execution.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum AutoResolutionPolicy {
    /// Prefer temporally more recent memory/document.
    #[default]
    PreferRecent,
    /// Prefer explicitly designated authoritative source.
    PreferAuthoritative,
}

/// Strategy mode used to execute a stewardship resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionStrategy {
    /// Automated policy execution.
    Automatic(AutoResolutionPolicy),
    /// Explicit human decision.
    Manual,
}

/// Current status of an applied resolution.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum ResolutionStatus {
    /// Resolution is currently active and applied.
    #[default]
    Applied,
    /// Resolution has been undone or reverted.
    Reverted,
    /// Finding was dismissed without action.
    Dismissed,
}

/// Opaque newtype identifier for a resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResolutionId(pub uuid::Uuid);

impl Default for ResolutionId {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolutionId {
    /// Generates a new random ResolutionId.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for ResolutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "res-{}", self.0)
    }
}

/// Applied resolution record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StewardshipResolution {
    /// Unique resolution identifier.
    pub id: ResolutionId,
    /// Associated finding identifier.
    pub finding_id: FindingId,
    /// Optional recommendation identifier.
    pub recommendation_id: Option<RecommendationId>,
    /// Execution strategy mode.
    pub strategy: ResolutionStrategy,
    /// Current resolution status.
    pub status: ResolutionStatus,
}

impl StewardshipResolution {
    /// Creates a new StewardshipResolution.
    pub fn new(
        finding_id: FindingId,
        recommendation_id: Option<RecommendationId>,
        strategy: ResolutionStrategy,
    ) -> Self {
        Self {
            id: ResolutionId::new(),
            finding_id,
            recommendation_id,
            strategy,
            status: ResolutionStatus::Applied,
        }
    }

    /// Reverts the resolution.
    pub fn revert(&mut self) {
        self.status = ResolutionStatus::Reverted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolution_revert_lifecycle() {
        let finding_id = FindingId::new();
        let mut res = StewardshipResolution::new(finding_id, None, ResolutionStrategy::Manual);
        assert_eq!(res.status, ResolutionStatus::Applied);

        res.revert();
        assert_eq!(res.status, ResolutionStatus::Reverted);
    }
}
