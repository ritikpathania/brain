//! SQLite storage driver and domain repository implementations.

#![deny(missing_docs)]

/// Storage error models.
pub mod errors;

/// Transaction boundary abstraction.
pub mod transaction;

/// Storage traits for EventStore, CheckpointStore, SnapshotStore.
pub mod traits;

/// Reference in-memory storage implementations.
pub mod in_memory;

/// Persistent SQLite implementations of CheckpointStore, SnapshotStore, and EventStore.
pub mod sqlite_store;

/// Durable WAL append-only event log backend and CRC32 verification.
pub mod wal_log;

/// Lifecycle policies and storage lifecycle orchestrator (Phase G Milestone G4).
pub mod policies;

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

pub use errors::*;
pub use event_log::{EventLogRepository, SqliteEventLog, StoredEvent};
pub use in_memory::*;
pub use jobs_projection::{JobReadModel, SqliteJobReadModelRepository};
pub use policies::*;
pub use projection_checkpoint::{
    ProjectionMetadataRecord, ProjectionStatus, SqliteProjectionCheckpointRepository,
    SqliteProjectionMetadataRepository,
};
pub use search_projection::{SearchQuery, SqliteSearchRepository};
pub use sessions_projection::{
    ReadModelRepository, SessionReadModel, SqliteSessionReadModelRepository,
};
pub use sqlite_store::*;
pub use store::SqliteStorage;
pub use traits::*;
pub use transaction::*;
pub use wal_log::*;
