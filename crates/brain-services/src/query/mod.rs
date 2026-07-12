//! Query services and DTO layer for retrieving and composing projection read models.

/// Data transfer objects.
pub mod dto;
/// Query filters and pagination specifications.
pub mod filters;
/// Query service interfaces.
pub mod traits;
/// Background jobs query service implementation.
pub mod jobs;
/// Sessions query service implementation.
pub mod sessions;
/// Search query service implementation.
pub mod search;
/// Shared subscription invalidation registry.
pub mod registry;
/// Live query DTO streams.
pub mod subscription;

pub use dto::{JobSummary, JobDetails, SessionSummary, SessionDetails, MessageDTO, SearchSummary};
pub use filters::{PaginationSpec, JobQuery, SessionQuery, SearchQuery};
pub use traits::{JobQueryService, SessionQueryService, SearchQueryService};
pub use jobs::SqliteJobQueryService;
pub use sessions::SqliteSessionQueryService;
pub use search::SqliteSearchQueryService;
pub use registry::{SubscriptionKey, QueryResponse, QuerySubscriptionRegistry};
pub use subscription::{
    QuerySnapshot, LiveQuery, WatchLiveQuery,
    SessionSubscriptionService, JobSubscriptionService, SearchSubscriptionService,
    SqliteSessionSubscriptionService, SqliteJobSubscriptionService, SqliteSearchSubscriptionService
};
