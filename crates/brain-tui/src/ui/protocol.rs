//! Communication protocol types for the TUI Client and Rust Daemon.

use crate::ui::interaction::MessageId;

/// Type-safe, opaque backend request identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(pub(crate) u64);

impl RequestId {
    /// Instantiates a new RequestId.
    pub fn new(val: u64) -> Self {
        Self(val)
    }
}

/// Sequential request identifier allocator.
#[derive(Debug, Default)]
pub struct RequestAllocator {
    next: u64,
}

impl RequestAllocator {
    /// Instantiates a new RequestAllocator.
    pub fn new() -> Self {
        Self { next: 1 }
    }

    /// Allocates a new unique sequential RequestId.
    pub fn next_id(&mut self) -> RequestId {
        let id = RequestId::new(self.next);
        self.next += 1;
        id
    }
}

/// Streaming completion classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishReason {
    /// Token generation ended successfully.
    Completed,
    /// Generation was explicitly cancelled.
    Cancelled,
    /// Generation failed.
    Error,
}

/// Downstream requests sent from TUI Client to Daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendCommand {
    /// Request response generation for prompt.
    SubmitPrompt {
        /// Associated backend request identifier.
        request: RequestId,
        /// Associated user message id.
        message: MessageId,
        /// Context string.
        text: String,
    },
    /// Request generator cancellation.
    Cancel {
        /// Associated message id.
        message: MessageId,
    },
    /// Request session rename.
    RenameSession {
        /// Session identifier.
        session_id: brain_domain::SessionId,
        /// New title value or None if cleared.
        title: Option<String>,
    },
    /// Request toggle pin.
    TogglePinSession {
        /// Session identifier.
        session_id: brain_domain::SessionId,
    },
    /// Request archive session.
    ArchiveSession {
        /// Session identifier.
        session_id: brain_domain::SessionId,
    },
    /// Request delete session.
    DeleteSession {
        /// Session identifier.
        session_id: brain_domain::SessionId,
    },
    /// Request restore session.
    RestoreSession {
        /// Session identifier.
        session_id: brain_domain::SessionId,
    },
    /// Reply to a tool call authorization request.
    ApproveToolCall {
        /// Unique identifier for the tool call.
        call_id: brain_core::events::ToolCallId,
        /// True if the tool execution is approved.
        approved: bool,
    },
}

/// Upstream responses returned from Daemon to TUI Client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendEvent {
    /// Append-only token event.
    Token {
        /// Message being updated.
        message: MessageId,
        /// Monotonic sequence index.
        sequence: u64,
        /// Dynamic chunk string.
        text: String,
    },
    /// Completed generation notification.
    Finished {
        /// Message finished.
        message: MessageId,
        /// Outcome classification.
        reason: FinishReason,
    },
    /// Backend request to authorize tool call.
    ToolCallRequest {
        /// Message requesting tool call.
        message: MessageId,
        /// Unique identifier for the tool call.
        call_id: brain_core::events::ToolCallId,
        /// The tool identifier.
        tool_id: brain_core::events::ToolId,
        /// The arguments payload string.
        arguments: String,
        /// Whether approval is required.
        requires_approval: bool,
    },
    /// Tool progress logging events.
    ToolProgress {
        /// Message related to tool execution.
        message: MessageId,
        /// Unique identifier for the tool call.
        call_id: brain_core::events::ToolCallId,
        /// Monotonic sequence index of progress event.
        sequence: u64,
        /// Progress completed metrics.
        detail: brain_core::events::ToolProgressDetail,
        /// Diagnostic message.
        log_message: String,
    },
    /// Final result output of a tool call.
    ToolCallResult {
        /// Message related to tool execution.
        message: MessageId,
        /// Unique identifier for the tool call.
        call_id: brain_core::events::ToolCallId,
        /// Tool result output content.
        result: String,
        /// True if execution resulted in error.
        is_error: bool,
    },
    /// Retrieval phase has started.
    RetrievalStarted {
        /// Message requesting retrieval.
        message: MessageId,
        /// The query text being searched.
        query: String,
    },
    /// Retrieved context info packet.
    RetrievalRetrieved {
        /// Message requesting retrieval.
        message: MessageId,
        /// Detailed user-facing retrieval entry.
        info: brain_domain::bkf::retrieval::RetrievalInfo,
    },
    /// Retrieval phase completed successfully.
    RetrievalCompleted {
        /// Message requesting retrieval.
        message: MessageId,
    },
    /// Stream update containing the latest sessions list.
    SessionsUpdated {
        /// The list of session summaries.
        sessions: Vec<crate::client::SessionSummary>,
    },
}
