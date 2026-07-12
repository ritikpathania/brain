use brain_domain::jobs::{JobId, JobKind, JobOwner, JobState, JobPriority, JobProgress, JobTimestamp, JobFailureReason};
use brain_domain::{SessionId, SessionTitle, SessionTimestamp, MessageId, MessageRole, MessageTimestamp, SearchDocumentId, SearchDocumentKind, SearchMetadata};

/// Short summary DTO representing a background job state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct JobSummary {
    /// Unique job ID.
    pub job_id: JobId,
    /// Kind classification.
    pub kind: JobKind,
    /// Owner context.
    pub owner: JobOwner,
    /// State status.
    pub state: JobState,
    /// Priority level.
    pub priority: JobPriority,
    /// Current determinate or indeterminate progress.
    pub progress: JobProgress,
}

/// Detailed DTO representing comprehensive background job metrics and telemetry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct JobDetails {
    /// Unique job ID.
    pub job_id: JobId,
    /// Kind classification.
    pub kind: JobKind,
    /// Owner context.
    pub owner: JobOwner,
    /// State status.
    pub state: JobState,
    /// Priority level.
    pub priority: JobPriority,
    /// Current determinate or indeterminate progress.
    pub progress: JobProgress,
    /// Time when execution started.
    pub started_at: Option<JobTimestamp>,
    /// Time when execution completed/terminated.
    pub completed_at: Option<JobTimestamp>,
    /// Reason for failure.
    pub failure_reason: Option<JobFailureReason>,
}

/// Short summary DTO representing a session read model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SessionSummary {
    /// Unique session ID.
    pub session_id: SessionId,
    /// Title metadata.
    pub title: SessionTitle,
    /// Whether the session is archived.
    pub is_archived: bool,
    /// Whether the session is pinned.
    pub is_pinned: bool,
    /// Creation timestamp.
    pub created_at: SessionTimestamp,
    /// Last update timestamp.
    pub updated_at: SessionTimestamp,
}

/// Detailed DTO representing a session along with its inner message logs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SessionDetails {
    /// Unique session ID.
    pub session_id: SessionId,
    /// Title metadata.
    pub title: SessionTitle,
    /// Whether the session is archived.
    pub is_archived: bool,
    /// Whether the session is pinned.
    pub is_pinned: bool,
    /// Creation timestamp.
    pub created_at: SessionTimestamp,
    /// Last update timestamp.
    pub updated_at: SessionTimestamp,
    /// Complete thread message history.
    pub messages: Vec<MessageDTO>,
}

/// DTO representing an individual message inside a session details thread.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct MessageDTO {
    /// Unique message ID.
    pub id: MessageId,
    /// Sender role.
    pub role: MessageRole,
    /// Text content.
    pub content: String,
    /// Timestamp of when the message was sent.
    pub timestamp: MessageTimestamp,
}

/// DTO representing an indexed unit returned from search index matching.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SearchSummary {
    /// Unique search document ID.
    pub id: SearchDocumentId,
    /// Kind classification.
    pub kind: SearchDocumentKind,
    /// Title field (if applicable).
    pub title: String,
    /// Body/Content excerpt matched.
    pub body: String,
    /// Structured metadata details.
    pub metadata: SearchMetadata,
}
