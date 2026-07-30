//! Strongly typed query error hierarchy.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Domain error during query analysis or compilation.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum QueryError {
    /// Semantic validation error.
    #[error("Semantic error: {message}")]
    Semantic {
        /// Detail message.
        message: String,
    },
    /// Duplicate variable binding.
    #[error("Duplicate variable: {var}")]
    DuplicateVariable {
        /// Variable name.
        var: String,
    },
}

/// Runtime error during physical execution.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum QueryExecutionError {
    /// Execution budget exceeded.
    #[error("Execution budget exceeded: {detail}")]
    BudgetExceeded {
        /// Detail.
        detail: String,
    },
    /// Query cancelled by caller.
    #[error("Query cancelled by user")]
    Cancelled,
    /// Physical operator error.
    #[error("Operator error: {message}")]
    OperatorFailed {
        /// Detail message.
        message: String,
    },
}
