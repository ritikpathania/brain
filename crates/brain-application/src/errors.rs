use thiserror::Error;

/// Semantic error classifications mapped from underlying domain/service layers.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ApplicationError {
    /// Validation error for input parameters.
    #[error("Validation failed: {0}")]
    Validation(String),

    /// Conflict error when state mutations collision occurs.
    #[error("Conflict detected: {0}")]
    Conflict(String),

    /// Service or resource unavailable.
    #[error("Service unavailable: {0}")]
    Unavailable(String),

    /// Task cancelled before completion.
    #[error("Operation was cancelled: {0}")]
    Cancelled(String),

    /// Timeout reached.
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Unrecoverable internal system error.
    #[error("Internal system error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for ApplicationError {
    fn from(err: serde_json::Error) -> Self {
        ApplicationError::Validation(err.to_string())
    }
}
