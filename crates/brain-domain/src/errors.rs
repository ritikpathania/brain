use thiserror::Error;

/// Domain-specific errors for the Brain domain model.
#[derive(Debug, Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DomainError {
    /// An edge already exists in the graph.
    #[error("Edge already exists: {source_node} -> {target_node} [{relation}]")]
    EdgeAlreadyExists {
        /// The source node ID.
        source_node: String,
        /// The target node ID.
        target_node: String,
        /// The relationship label.
        relation: String,
    },
    /// The source node of a relationship is missing.
    #[error("Missing source node: {0}")]
    MissingSourceNode(String),
    /// The target node of a relationship is missing.
    #[error("Missing target node: {0}")]
    MissingTargetNode(String),
    /// Session is archived and cannot be modified.
    #[error("Session is archived: {0}")]
    SessionArchived(String),
    /// Goal already exists in the session.
    #[error("Duplicate goal in session: {0}")]
    DuplicateGoal(String),
    /// Goal was not found in the session.
    #[error("Goal not found in session: {0}")]
    GoalNotFound(String),
    /// Invalid edge weight specified.
    #[error("Invalid edge weight: {0}")]
    InvalidEdgeWeight(String),
    /// Invalid relation type or unregistered relation ID.
    #[error("Unregistered relation: {0}")]
    UnregisteredRelation(String),
    /// Domain validation invariant failure.
    #[error("Validation error: {message}")]
    ValidationError {
        /// Failure message text.
        message: String,
        /// Invariant rule identifier code.
        rule_id: Option<String>,
    },
}
