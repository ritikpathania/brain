//! Storage transaction boundary abstraction.

use crate::errors::StorageError;

/// Minimal transaction boundary abstraction for atomic storage operations.
pub trait Transaction: Send {
    /// Commits the atomic transaction.
    fn commit(self: Box<Self>) -> Result<(), StorageError>;

    /// Aborts and rolls back the atomic transaction.
    fn rollback(self: Box<Self>) -> Result<(), StorageError>;
}
