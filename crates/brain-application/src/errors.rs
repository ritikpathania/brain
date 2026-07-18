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

impl From<brain_core::errors::BrainError> for ApplicationError {
    fn from(err: brain_core::errors::BrainError) -> Self {
        match err {
            brain_core::errors::BrainError::Validation { message } => {
                ApplicationError::Validation(message)
            }
            brain_core::errors::BrainError::Authorization { message } => {
                ApplicationError::Unavailable(format!("Unauthorized: {}", message))
            }
            brain_core::errors::BrainError::Timeout { message, .. } => {
                ApplicationError::Timeout(message)
            }
            brain_core::errors::BrainError::Cancelled { message } => {
                ApplicationError::Cancelled(message)
            }
            brain_core::errors::BrainError::Session { message, .. } => {
                ApplicationError::Conflict(message)
            }
            brain_core::errors::BrainError::Configuration { message } => {
                ApplicationError::Internal(format!("Configuration error: {}", message))
            }
            brain_core::errors::BrainError::InvalidTransition { message } => {
                ApplicationError::Conflict(message)
            }
            brain_core::errors::BrainError::Internal { message } => {
                ApplicationError::Internal(message)
            }
            brain_core::errors::BrainError::Storage { message, .. } => {
                ApplicationError::Internal(format!("Storage error: {}", message))
            }
            brain_core::errors::BrainError::Plugin { message, .. } => {
                ApplicationError::Internal(format!("Plugin error: {}", message))
            }
            brain_core::errors::BrainError::Python { message, .. } => {
                ApplicationError::Internal(format!("Python error: {}", message))
            }
            brain_core::errors::BrainError::Network { message, .. } => {
                ApplicationError::Unavailable(format!("Network error: {}", message))
            }
            brain_core::errors::BrainError::Tool { message, .. } => {
                ApplicationError::Internal(format!("Tool error: {}", message))
            }
        }
    }
}
