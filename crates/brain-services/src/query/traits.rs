use crate::query::dto::{JobSummary, JobDetails, SessionSummary, SessionDetails, SearchSummary};
use crate::query::filters::{JobQuery, SessionQuery, SearchQuery};

/// Service providing query and pagination interfaces for background job read models.
pub trait JobQueryService: Send + Sync {
    /// Lists job summaries matching the given filters and pagination specifications.
    fn list_jobs(&self, query: JobQuery) -> Result<Vec<JobSummary>, brain_core::errors::BrainError>;
    /// Retrieves full details of a specific job by its ID.
    fn get_job(&self, id: &brain_domain::jobs::JobId) -> Result<Option<JobDetails>, brain_core::errors::BrainError>;
}

/// Service providing query and pagination interfaces for session read models and history logs.
pub trait SessionQueryService: Send + Sync {
    /// Lists session summaries matching the given filters and pagination specifications.
    fn list_sessions(&self, query: SessionQuery) -> Result<Vec<SessionSummary>, brain_core::errors::BrainError>;
    /// Retrieves full details of a specific session (including message logs) by its ID.
    fn get_session(&self, id: &brain_domain::SessionId) -> Result<Option<SessionDetails>, brain_core::errors::BrainError>;
}

/// Service providing candidate text search matches from the FTS5 projection read model.
pub trait SearchQueryService: Send + Sync {
    /// Searches candidates in the FTS5 projection matching the given search query parameters.
    fn search(&self, query: SearchQuery) -> Result<Vec<SearchSummary>, brain_core::errors::BrainError>;
}
