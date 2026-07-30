//! Strongly-typed Query Errors for Phase 5 Query Facade.

use thiserror::Error;

/// Strongly-typed query errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// Requested entity was not found in knowledge read models.
    #[error("Entity not found: {0}")]
    EntityNotFound(String),
    /// Invalid input query parameters.
    #[error("Invalid query parameters: {0}")]
    InvalidParameters(String),
    /// Query execution timed out.
    #[error("Query timeout exceeded after {0} ms")]
    Timeout(u64),
    /// Requested query operation or capability is unsupported.
    #[error("Unsupported query capability: {0}")]
    UnsupportedQuery(String),
    /// Internal query evaluation failure.
    #[error("Query evaluation failed: {0}")]
    EvaluationFailed(String),
}
