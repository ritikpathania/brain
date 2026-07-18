//! SQLite storage driver and domain repository implementations.

#![deny(missing_docs)]

/// Connection pooling module.
pub mod connection;

/// Schema setup and migration coordinator.
pub mod migrations;

/// Private SQLite repository implementations.
pub mod store;

/// SQLite event log implementations.
pub mod event_log;

/// Projection checkpoint repository implementation.
pub mod projection_checkpoint;

/// SQLite jobs projection read model.
pub mod jobs_projection;

/// SQLite sessions projection read model.
pub mod sessions_projection;

/// SQLite search index FTS5 projection read model.
pub mod search_projection;

/// Re-export test utilities for test environments or when `test-utils` feature is active.
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

#[cfg(any(test, feature = "test-utils"))]
pub use test_utils::TestStorage;

pub use event_log::{EventLogRepository, SqliteEventLog, StoredEvent};
pub use jobs_projection::{JobReadModel, SqliteJobReadModelRepository};
pub use projection_checkpoint::SqliteProjectionCheckpointRepository;
pub use search_projection::{SearchQuery, SqliteSearchRepository};
pub use sessions_projection::{
    ReadModelRepository, SessionReadModel, SqliteSessionReadModelRepository,
};
pub use store::SqliteStorage;
