//! Storage and transaction error definitions for persistent storage operations.

use thiserror::Error;

/// Storage operation errors returned by `brain-storage` implementations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// Requested record or entity was not found.
    #[error("Storage record not found: {0}")]
    NotFound(String),

    /// Transaction execution or boundary failure.
    #[error("Storage transaction error: {0}")]
    Transaction(String),

    /// Data serialization or deserialization failure.
    #[error("Storage serialization error: {0}")]
    Serialization(String),

    /// Internal storage system error.
    #[error("Internal storage error: {0}")]
    Internal(String),
}

/// Transaction boundary commit or rollback errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransactionError {
    /// Transaction commit failed.
    #[error("Transaction commit failed: {0}")]
    CommitFailed(String),

    /// Transaction rollback failed.
    #[error("Transaction rollback failed: {0}")]
    RollbackFailed(String),
}
