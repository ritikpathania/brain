use crate::jobs::{
    JobId, JobKind, JobPriority, JobOwner, JobTimestamp, JobProgress, JobFailureReason, ArtifactId, ArtifactKind, LogEntryId
};

use crate::identifiers::{SessionId, SessionTitle, SessionTimestamp};
use crate::entities::MessageSnapshot;

/// Domain events emitted by the Brain domain model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum DomainEvent {
    /// A new memory node has been created.
    MemoryCreated {
        /// The unique ID of the created node.
        node_id: String,
    },
    /// Two memory nodes have been merged.
    MemoryMerged {
        /// The target node ID that remains.
        target_id: String,
        /// The node ID that was merged/absorbed.
        merged_id: String,
    },
    /// A memory node has been promoted (e.g. to a higher status or importance).
    MemoryPromoted {
        /// The promoted node ID.
        node_id: String,
        /// Reason for promotion.
        reason: String,
    },
    /// A memory node has been forgotten.
    MemoryForgotten {
        /// The forgotten node ID.
        node_id: String,
    },
    /// A new session has been created.
    SessionCreated {
        /// The created session ID.
        session_id: SessionId,
        /// The title of the session.
        title: SessionTitle,
        /// The created timestamp.
        created_at: SessionTimestamp,
    },
    /// A session has been renamed.
    SessionRenamed {
        /// The renamed session ID.
        session_id: SessionId,
        /// The new title.
        title: SessionTitle,
        /// The updated timestamp.
        updated_at: SessionTimestamp,
    },
    /// A session's pinning status changed.
    SessionPinnedChanged {
        /// The session ID.
        session_id: SessionId,
        /// The new pinned status.
        pinned: bool,
        /// The updated timestamp.
        updated_at: SessionTimestamp,
    },
    /// A session has been archived.
    SessionArchived {
        /// The archived session ID.
        session_id: SessionId,
        /// The updated timestamp.
        updated_at: SessionTimestamp,
    },
    /// An archived session has been restored.
    SessionRestored {
        /// The restored session ID.
        session_id: SessionId,
        /// The updated timestamp.
        updated_at: SessionTimestamp,
    },
    /// A session has been deleted.
    SessionDeleted {
        /// The deleted session ID.
        session_id: SessionId,
    },
    /// A chat message has been added to a session.
    MessageAdded {
        /// Parent session ID.
        session_id: SessionId,
        /// Message snapshot details.
        message: MessageSnapshot,
    },
    /// A relationship edge has been reinforced/strengthened.
    RelationshipStrengthened {
        /// The source node ID.
        source: String,
        /// The target node ID.
        target: String,
        /// The relationship label.
        relation: String,
        /// The new weight of the relationship.
        new_weight: f64,
    },
    /// A new background job was created.
    JobCreated {
        /// Job identity.
        job_id: JobId,
        /// Kind classification.
        kind: JobKind,
        /// Precedence priority tier.
        priority: JobPriority,
        /// Aggregate owner context.
        owner: JobOwner,
    },
    /// Job execution started.
    JobStarted {
        /// Job identity.
        job_id: JobId,
        /// Starting timestamp.
        timestamp: JobTimestamp,
    },
    /// Job progress updated.
    JobProgressed {
        /// Job identity.
        job_id: JobId,
        /// New progress state.
        progress: JobProgress,
    },
    /// Job transitioned to waiting.
    JobWaiting {
        /// Job identity.
        job_id: JobId,
        /// Reentry blocking reason.
        reason: String,
    },
    /// Job completed successfully.
    JobCompleted {
        /// Job identity.
        job_id: JobId,
        /// Ending timestamp.
        timestamp: JobTimestamp,
    },
    /// Job execution failed.
    JobFailed {
        /// Job identity.
        job_id: JobId,
        /// Failure reason details.
        reason: JobFailureReason,
        /// Terminal timestamp.
        timestamp: JobTimestamp,
    },
    /// Job execution cancelled.
    JobCancelled {
        /// Job identity.
        job_id: JobId,
        /// Abort timestamp.
        timestamp: JobTimestamp,
    },
    /// Artifact produced during execution.
    ArtifactProduced {
        /// Job identity.
        job_id: JobId,
        /// Artifact identity.
        artifact_id: ArtifactId,
        /// Type category of artifact.
        kind: ArtifactKind,
    },
    /// Diagnostic log trace appended.
    LogAppended {
        /// Job identity.
        job_id: JobId,
        /// Monotonic trace sequence index.
        entry_id: LogEntryId,
        /// Trace message content.
        message: String,
    },
    /// An observation was received by KPP.
    KppObservationReceived {
        /// Observation ID.
        id: String,
        /// Source classification description.
        source: String,
    },
    /// An observation was parsed into raw pre-optimization KnowledgeIR.
    KppObservationParsed {
        /// Observation ID.
        id: String,
        /// Number of nodes parsed.
        nodes_count: usize,
        /// Number of edges parsed.
        edges_count: usize,
    },
    /// KPP graph compilation has started.
    KppCompilationStarted {
        /// Observation ID.
        id: String,
    },
    /// KPP graph compilation has completed.
    KppCompilationCompleted {
        /// Observation ID.
        id: String,
        /// Number of nodes compiled.
        nodes_count: usize,
        /// Number of edges compiled.
        edges_count: usize,
        /// Number of compiler diagnostics.
        diagnostics_count: usize,
    },
    /// KPP graph optimization has completed.
    KppOptimizationCompleted {
        /// Observation ID.
        id: String,
        /// Number of optimized nodes.
        nodes_count: usize,
        /// Number of optimized edges.
        edges_count: usize,
        /// Number of optimizer diagnostics.
        diagnostics_count: usize,
    },
    /// Idempotent projection deltas were calculated.
    KppProjectionCalculated {
        /// Observation ID.
        id: String,
        /// Number of calculated SQLite operations.
        sqlite_ops_count: usize,
    },
    /// Projection deltas were transactionally applied.
    KppProjectionApplied {
        /// Observation ID.
        id: String,
    },
}
