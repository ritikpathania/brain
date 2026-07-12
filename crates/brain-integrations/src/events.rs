//! Ingestion event structures and taxonomy registries.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use specta::Type;

/// A transport-agnostic value tree, matching standard JSON/CBOR layouts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[serde(untagged)]
pub enum Value {
    /// A null value.
    Null,
    /// A boolean value.
    Bool(bool),
    /// A floating-point number.
    Number(f64),
    /// A UTF-8 string value.
    String(String),
    /// An array of nested Values.
    Array(Vec<Value>),
    /// A key-value map of nested Values.
    Object(BTreeMap<String, Value>),
}

/// Opaque metadata map. Brain indexes by key for filtering but never
/// interprets the values semantically.
pub type EventMetadata = std::collections::BTreeMap<String, Value>;

/// Stable identifiers for event types, decoupled from variant names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Ingestion message event.
    Message,
    /// Ingestion tool call event.
    ToolCall,
    /// Ingestion tool result event.
    ToolResult,
    /// File edit workspace event.
    FileEdit,
    /// Diagnostic workspace event.
    Diagnostic,
    /// Terminal command execution event.
    TerminalCommand,
    /// Git commit workspace event.
    GitCommit,
    /// Git branch lifecycle event.
    GitBranch,
    /// System session started lifecycle event.
    SessionStarted,
    /// System session ended lifecycle event.
    SessionEnded,
    /// System adapter connected connection event.
    AdapterConnected,
    /// System adapter disconnected connection event.
    AdapterDisconnected,
    /// Unstructured text fallback event.
    Text,
}

/// Structured capability identifiers for negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Ingestion of conversational turns.
    ConversationMessages,
    /// Ingestion of assistant tool calls & results.
    ConversationTools,
    /// Ingestion of git branch shifts and commits.
    WorkspaceGit,
    /// Ingestion of workspace file modification diffs.
    WorkspaceFiles,
    /// Ingestion of shell command traces.
    WorkspaceTerminal,
    /// Ingestion of editor diagnostics/compilation alerts.
    WorkspaceDiagnostics,
    /// Support for event replay sequences.
    Replay,
    /// Support for multi-event batch submissions.
    Batching,
}

/// Tagged union of all ingestion event types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[serde(tag = "event_type")]
pub enum IngestionEvent {
    /// A conversation message from user, assistant or system.
    #[serde(rename = "message")]
    Message {
        /// Message text content.
        content: String,
        /// Opaque event metadata.
        metadata: EventMetadata,
        /// Sender role (e.g. "user", "assistant", "system").
        role: String,
    },

    /// An assistant-initiated tool invocation.
    #[serde(rename = "tool_call")]
    ToolCall {
        /// Arguments map passed to the tool.
        arguments: Value,
        /// Call identifier correlating to the request.
        call_id: String,
        /// Opaque event metadata.
        metadata: EventMetadata,
        /// Name of the called tool.
        tool_name: String,
    },

    /// The response/output returned from a tool execution.
    #[serde(rename = "tool_result")]
    ToolResult {
        /// Correlation identifier matching the ToolCall.
        call_id: String,
        /// Flag set to true if execution failed.
        is_error: bool,
        /// Opaque event metadata.
        metadata: EventMetadata,
        /// Text stdout output or representation of tool results.
        output: String,
    },

    /// Workspace event indicating a file modification diff.
    #[serde(rename = "file_edit")]
    FileEdit {
        /// Unified diff representing changes made, or None if unknown.
        diff: Option<String>,
        /// Opaque event metadata.
        metadata: EventMetadata,
        /// Path of the file edited.
        path: String,
    },

    /// Compilation/linter diagnostic or type-check warning.
    #[serde(rename = "diagnostic")]
    Diagnostic {
        /// File path where diagnostic occurred.
        file: Option<String>,
        /// 1-indexed line index, or None if unknown.
        line: Option<u32>,
        /// Plaintext diagnostic details.
        message: String,
        /// Opaque event metadata.
        metadata: EventMetadata,
        /// Diagnostic severity (e.g. "error", "warning", "info", "hint").
        severity: String,
        /// Source identifier (e.g. "rustc", "eslint").
        source: String,
    },

    /// Shell command execution trace.
    #[serde(rename = "terminal_command")]
    TerminalCommand {
        /// Full CLI string executed.
        command: String,
        /// Exit code returned by the shell process.
        exit_code: Option<i32>,
        /// Opaque event metadata.
        metadata: EventMetadata,
        /// Truncated stdout string or summary of output.
        stdout_summary: Option<String>,
    },

    /// Git commit snapshot to version control.
    #[serde(rename = "git_commit")]
    GitCommit {
        /// Active branch name at the time of commit.
        branch: Option<String>,
        /// List of workspace-relative files modified in this commit.
        files_changed: Vec<String>,
        /// SHA-1 commit hash.
        hash: String,
        /// Message text of the commit.
        message: String,
        /// Opaque event metadata.
        metadata: EventMetadata,
    },

    /// Git branch lifecycle change.
    #[serde(rename = "git_branch")]
    GitBranch {
        /// Lifecycle action (e.g. "create", "switch", "merge", "delete").
        action: String,
        /// Branch name targeted.
        branch_name: String,
        /// Opaque event metadata.
        metadata: EventMetadata,
    },

    /// Ingestion session started signal.
    #[serde(rename = "session_started")]
    SessionStarted {
        /// Opaque session metadata.
        metadata: EventMetadata,
    },

    /// Ingestion session ended signal.
    #[serde(rename = "session_ended")]
    SessionEnded {
        /// Opaque session metadata.
        metadata: EventMetadata,
    },

    /// Handshake connection event for capability negotiation.
    #[serde(rename = "adapter_connected")]
    AdapterConnected {
        /// Client adapter version.
        adapter_version: String,
        /// Capability set advertised by the client.
        capabilities: Vec<Capability>,
        /// Opaque connection metadata.
        metadata: EventMetadata,
        /// Event Model Versions supported by the client.
        supported_event_model_versions: Vec<String>,
        /// Serializations supported by the client.
        supported_serializations: Vec<String>,
    },

    /// Client connection teardown notification.
    #[serde(rename = "adapter_disconnected")]
    AdapterDisconnected {
        /// Opaque connection metadata.
        metadata: EventMetadata,
        /// Reason string, or None if clean disconnect.
        reason: Option<String>,
    },

    /// Fallback freeform text ingestion payload.
    #[serde(rename = "text")]
    Text {
        /// Raw context string.
        content: String,
        /// Opaque event metadata.
        metadata: EventMetadata,
    },
}

impl IngestionEvent {
    /// Returns the stable `EventKind` of the event.
    pub fn kind(&self) -> EventKind {
        match self {
            Self::Message { .. } => EventKind::Message,
            Self::ToolCall { .. } => EventKind::ToolCall,
            Self::ToolResult { .. } => EventKind::ToolResult,
            Self::FileEdit { .. } => EventKind::FileEdit,
            Self::Diagnostic { .. } => EventKind::Diagnostic,
            Self::TerminalCommand { .. } => EventKind::TerminalCommand,
            Self::GitCommit { .. } => EventKind::GitCommit,
            Self::GitBranch { .. } => EventKind::GitBranch,
            Self::SessionStarted { .. } => EventKind::SessionStarted,
            Self::SessionEnded { .. } => EventKind::SessionEnded,
            Self::AdapterConnected { .. } => EventKind::AdapterConnected,
            Self::AdapterDisconnected { .. } => EventKind::AdapterDisconnected,
            Self::Text { .. } => EventKind::Text,
        }
    }
}
