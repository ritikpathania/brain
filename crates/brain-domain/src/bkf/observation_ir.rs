use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Source kinds for raw inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationSource {
    /// Ingested conversation message.
    Conversation {
        /// Parent conversation ID.
        conversation_id: String,
        /// Current session ID.
        session_id: String,
        /// Ingested user prompt.
        prompt: String,
        /// Optional assistant generated response.
        response: Option<String>,
    },
    /// Ingested tool execution step.
    Tool {
        /// Name of the executed tool.
        tool_name: String,
        /// Inputs passed to the tool.
        input: serde_json::Value,
        /// Outputs returned from the tool.
        output: serde_json::Value,
    },
    /// Ingested static file content.
    File {
        /// File path.
        path: String,
        /// Full file content.
        content: String,
        /// Last modified unix timestamp.
        modified_at: u64,
    },
    /// Ingested system operational events.
    System {
        /// Unique system event identifier/name.
        event_name: String,
        /// Arbitrary event parameters.
        details: HashMap<String, serde_json::Value>,
    },
}

/// Input conversation observation struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationObservation {
    /// Parent conversation ID.
    pub conversation_id: String,
    /// Current session ID.
    pub session_id: String,
    /// Ingested user prompt.
    pub prompt: String,
    /// Optional assistant response.
    pub response: Option<String>,
}

/// Input tool observation struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolObservation {
    /// Name of the executed tool.
    pub tool_name: String,
    /// Inputs passed to the tool.
    pub input: serde_json::Value,
    /// Outputs returned from the tool.
    pub output: serde_json::Value,
}

/// Input file observation struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileObservation {
    /// File path.
    pub path: String,
    /// Full file content.
    pub content: String,
    /// Last modified unix timestamp.
    pub modified_at: u64,
}

/// Input system observation struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemObservation {
    /// Unique system event identifier/name.
    pub event_name: String,
    /// Arbitrary event parameters.
    pub details: HashMap<String, serde_json::Value>,
}

/// Input wrapper for incoming source observations before parsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Observation {
    /// Conversation message.
    Conversation(ConversationObservation),
    /// Tool execution trace.
    Tool(ToolObservation),
    /// Static file content.
    File(FileObservation),
    /// System event.
    System(SystemObservation),
}

/// Raw, source-level input representation capturing all metadata, raw text, and context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationIR {
    /// Unique identity of the observation.
    pub id: String,
    /// Origin of this observation.
    pub source: ObservationSource,
    /// Monotonically incrementing timestamp when the observation occurred.
    pub timestamp: u64,
    /// Extensible metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ObservationIR {
    /// Parses an incoming `Observation` and returns a formatted `ObservationIR`.
    pub fn parse(
        id: String,
        timestamp: u64,
        observation: Observation,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        let source = match observation {
            Observation::Conversation(obs) => ObservationSource::Conversation {
                conversation_id: obs.conversation_id,
                session_id: obs.session_id,
                prompt: obs.prompt,
                response: obs.response,
            },
            Observation::Tool(obs) => ObservationSource::Tool {
                tool_name: obs.tool_name,
                input: obs.input,
                output: obs.output,
            },
            Observation::File(obs) => ObservationSource::File {
                path: obs.path,
                content: obs.content,
                modified_at: obs.modified_at,
            },
            Observation::System(obs) => ObservationSource::System {
                event_name: obs.event_name,
                details: obs.details,
            },
        };

        Self {
            id,
            source,
            timestamp,
            metadata,
        }
    }
}

/// Executed trace details used by Reflection Engine for post-compilation analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Trace correlation ID.
    pub trace_id: String,
    /// Original prompt triggering the trace.
    pub prompt: String,
    /// Success outcome of the execution.
    pub success: bool,
    /// Epoch of execution.
    pub epoch: u64,
    /// Timestamp of trace completion.
    pub timestamp: u64,
    /// Diagnostics/Traces recorded during execution steps.
    pub step_logs: Vec<String>,
}

