use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

/// Metadata carrying unique context for a stream event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Unique query execution identifier.
    pub execution_id: Uuid,
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// Epoch timestamp when the event was emitted.
    pub timestamp: SystemTime,
}

/// A sequential packet carrying execution progress or output chunks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Contextual metadata.
    pub metadata: EventMetadata,
    /// Stream payload category.
    pub kind: StreamEventKind,
}

/// Categories of streamed payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEventKind {
    /// A single token chunk of generated assistant text response.
    Token(String),
    /// Descriptive human-readable progress updates.
    Progress {
        /// Diagnostic progress text message.
        message: String,
        /// Monotonic completion percentage indicator.
        percentage: Option<f32>,
    },
    /// A transition signaling entering or leaving a pipeline execution stage.
    Stage {
        /// Symbolic name/identifier of the stage.
        name: String,
        /// Status indicator (true: entering, false: leaving).
        active: bool,
    },
    /// Successful completion event returning full response content.
    Finished {
        /// Final response content.
        response: String,
    },
    /// Execution cancelled signal.
    Cancelled,
    /// Execution error diagnostics.
    Error {
        /// Diagnostics error message description.
        message: String,
    },
    /// Request user authorization for a tool call.
    ToolCallRequest {
        /// Unique tool call ID.
        call_id: ToolCallId,
        /// Name / identifier of the tool.
        tool_id: ToolId,
        /// Arguments json payload.
        arguments: String,
        /// True if user approval is required before execution.
        requires_approval: bool,
    },
    /// Incremental progress log updates from a running tool.
    ToolProgress {
        /// Unique tool call ID.
        call_id: ToolCallId,
        /// Monotonic sequence within the tool call lifecycle.
        sequence: u64,
        /// Determinate or indeterminate progress metrics.
        detail: ToolProgressDetail,
        /// Diagnostic progress text message.
        message: String,
    },
    /// Final result of a tool execution.
    ToolCallResult {
        /// Unique tool call ID.
        call_id: ToolCallId,
        /// Output content or error description from the tool.
        result: String,
        /// True if execution failed.
        is_error: bool,
    },
    /// Context retrieval has started.
    RetrievalStarted {
        /// The query text being searched.
        query: String,
    },
    /// A matching chunk retrieved from local or remote knowledge base.
    RetrievalRetrieved {
        /// Detailed user-facing retrieval entry.
        info: brain_domain::bkf::retrieval::RetrievalInfo,
    },
    /// Retrieval phase completed.
    RetrievalCompleted,
}

/// Type-safe opaque identifier for a tool execution call.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCallId(pub String);


/// Type-safe opaque identifier for a tool descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId(pub String);

/// Unit category for tool progress completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgressUnit {
    /// Steps completed.
    Steps,
    /// Bytes processed.
    Bytes,
    /// Arbitrary items completed.
    Items,
}

/// Completion details for a tool execution step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolProgressDetail {
    /// Progress is ongoing but has no absolute total bound.
    Indeterminate,
    /// Progress has known completed and total quantities.
    Determinate {
        /// Number of units completed.
        completed: u64,
        /// Total number of units expected.
        total: u64,
        /// The progress unit kind.
        unit: ProgressUnit,
    },
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_json_roundtrip() {
        let meta = EventMetadata {
            execution_id: Uuid::new_v4(),
            sequence: 42,
            timestamp: SystemTime::now(),
        };
        let event = StreamEvent {
            metadata: meta,
            kind: StreamEventKind::Token("Hello".to_string()),
        };
        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: StreamEvent = serde_json::from_str(&serialized).unwrap();
        assert_eq!(event.metadata.sequence, deserialized.metadata.sequence);
        if let StreamEventKind::Token(token) = deserialized.kind {
            assert_eq!(token, "Hello");
        } else {
            panic!("Expected Token variant");
        }
    }
}
