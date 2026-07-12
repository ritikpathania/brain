//! Domain model and state representation for backend tool execution.

/// Re-export ToolCallId from brain_core.
pub use brain_core::events::ToolCallId;
/// Re-export ToolId from brain_core.
pub use brain_core::events::ToolId;
/// Re-export ToolProgressDetail from brain_core.
pub use brain_core::events::ToolProgressDetail;

use std::time::SystemTime;

/// Machine-readable state classification of a tool invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionStatus {
    /// Tool is awaiting user authorization/approval.
    PendingApproval,
    /// Tool call was approved but has not run or finished yet.
    Approved,
    /// Tool call was denied by the user.
    Denied,
    /// Tool is currently executing on the backend.
    Running {
        /// Structured details of step progress.
        progress: ToolProgressDetail,
    },
    /// Tool execution successfully completed.
    Completed {
        /// Returned output or value.
        result: String,
    },
    /// Tool execution failed.
    Failed {
        /// Diagnostic error description.
        error: String,
    },
}

impl ToolExecutionStatus {
    /// Returns true if the status represents a final outcome.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Denied | Self::Completed { .. } | Self::Failed { .. }
        )
    }
}

/// A human-readable log entry printed by a running tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolLogEntry {
    /// Timestamp when this log line was received or generated.
    pub timestamp: SystemTime,
    /// Log line content string.
    pub message: String,
}

/// Bookkeeping state tracking a protocol event sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProtocolState {
    /// The highest sequence number processed for this tool call.
    pub last_sequence: u64,
}

/// Pure domain state tracking the execution of a backend tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecution {
    /// Message ID that triggered the tool call.
    pub message_id: crate::ui::interaction::MessageId,
    /// Unique identifier for the tool call instance.
    pub call_id: ToolCallId,
    /// Identifier for the tool descriptor/definition.
    pub tool_id: ToolId,
    /// Machine-readable lifecycle status.
    pub status: ToolExecutionStatus,
    /// Chronological list of logs generated during execution.
    pub logs: Vec<ToolLogEntry>,
    /// Protocol event sequencing bookkeeping.
    pub protocol_state: ProtocolState,
}

impl ToolExecution {
    /// Creates a new `ToolExecution` in `PendingApproval` state.
    pub fn new(message_id: crate::ui::interaction::MessageId, call_id: ToolCallId, tool_id: ToolId) -> Self {
        Self {
            message_id,
            call_id,
            tool_id,
            status: ToolExecutionStatus::PendingApproval,
            logs: Vec::new(),
            protocol_state: ProtocolState::default(),
        }
    }
}

/// Model capturing a pending tool call authorization request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolApproval {
    /// Message ID that triggered the tool call.
    pub message_id: crate::ui::interaction::MessageId,
    /// Unique identifier for the tool call request.
    pub call_id: ToolCallId,
    /// Identifier of the requested tool.
    pub tool_id: ToolId,
    /// Unparsed JSON string of arguments.
    pub arguments: String,
}

