//! Query services and DTO layer for retrieving and composing projection read models.

/// Data transfer objects.
pub mod dto;
/// Query filters and pagination specifications.
pub mod filters;
/// Background jobs query service implementation.
pub mod jobs;
/// Shared subscription invalidation registry.
pub mod registry;
/// Search query service implementation.
pub mod search;
/// Sessions query service implementation.
pub mod sessions;
/// Live query DTO streams.
pub mod subscription;
/// Query service interfaces.
pub mod traits;

pub use dto::{JobDetails, JobSummary, MessageDTO, SearchSummary, SessionDetails, SessionSummary};
pub use filters::{JobQuery, PaginationSpec, SearchQuery, SessionQuery};
pub use jobs::SqliteJobQueryService;
pub use registry::{QueryResponse, QuerySubscriptionRegistry, SubscriptionKey};
pub use search::SqliteSearchQueryService;
pub use sessions::SqliteSessionQueryService;
pub use subscription::{
    JobSubscriptionService, LiveQuery, QuerySnapshot, SearchSubscriptionService,
    SessionSubscriptionService, SqliteJobSubscriptionService, SqliteSearchSubscriptionService,
    SqliteSessionSubscriptionService, WatchLiveQuery,
};
pub use traits::{JobQueryService, SearchQueryService, SessionQueryService};
