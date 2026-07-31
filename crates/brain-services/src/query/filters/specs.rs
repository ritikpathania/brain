//! Read-model query filter DTO specifications.

use brain_domain::jobs::{JobOwner, JobState};
use brain_domain::SearchDocumentKind;

/// Specifications for pagination in read-model lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PaginationSpec {
    /// Maximum number of items to return.
    pub limit: Option<usize>,
    /// Number of items to skip before returning.
    pub offset: Option<usize>,
}

/// Specifications for filtering background job queries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JobQuery {
    /// Optional job execution owner filter.
    pub owner: Option<JobOwner>,
    /// Optional job status state filter.
    pub state: Option<JobState>,
    /// Optional pagination limits.
    pub pagination: Option<PaginationSpec>,
}

/// Specifications for filtering session read model queries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SessionQuery {
    /// Optional archived status filter.
    pub is_archived: Option<bool>,
    /// Optional pinned status filter.
    pub is_pinned: Option<bool>,
    /// Optional pagination limits.
    pub pagination: Option<PaginationSpec>,
}

/// Specifications for filtering search queries at the service layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SearchQuery {
    /// The query matching text.
    pub text: String,
    /// Optional filter by document kinds.
    pub kinds: Option<Vec<SearchDocumentKind>>,
    /// Optional pagination limits.
    pub pagination: Option<PaginationSpec>,
}
