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
