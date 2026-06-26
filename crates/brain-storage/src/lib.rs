//! SQLite storage driver and domain repository implementations.

#![deny(missing_docs)]

/// Connection pooling module.
pub mod connection;

/// Schema setup and migration coordinator.
pub mod migrations;

/// Private SQLite repository implementations.
pub mod store;

pub use store::SqliteStorage;
