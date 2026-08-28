use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LegacyRequest {
    pub action: String,
    pub payload: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VersionedRequest {
    pub version: String,
    #[serde(rename = "type")]
    pub msg_type: String, // "Request"
    pub id: u64,
    pub action: String,
    /// Plain prompt text. Type unchanged for backward compatibility with old daemons.
    pub body: String,
    /// Optional workspace node IDs supplied by the TUI client.
    /// Old daemons that do not know this field will ignore it (serde default = empty Vec).
    /// Omitted from serialisation when empty to keep clean wire frames for old clients.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_context: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct JsonRpcRequest {
    pub id: serde_json::Value,
    pub action: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum ClientRequest {
    Versioned(VersionedRequest),
    Legacy(LegacyRequest),
    JsonRpc(JsonRpcRequest),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionedResponse {
    pub version: String,
    #[serde(rename = "type")]
    pub msg_type: String, // "Response"
    pub id: u64,
    pub status: String,
    pub body: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionedError {
    pub version: String,
    #[serde(rename = "type")]
    pub msg_type: String, // "Error"
    pub id: u64,
    pub status: String,
    pub body: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionedEvent {
    pub version: String,
    #[serde(rename = "type")]
    pub msg_type: String, // "Event"
    pub event_name: String,
    pub payload: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionedNotification {
    pub version: String,
    #[serde(rename = "type")]
    pub msg_type: String, // "Notification"
    pub notification_type: String,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExtractedEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryResultNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub content: String,
    pub attributes: serde_json::Value,
    pub score: i64,
    pub source: String,
    pub connections: Vec<ExtractedEdge>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LegacyResponse {
    pub status: String,
    pub message: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StartSessionPayload {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(rename = "systemPrompt", alias = "system_prompt", default)]
    pub system_prompt: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StartSessionResponseBody {
    pub session_id: String,
    pub title: String,
    pub created_at_ms: i64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AppendTurnPayload {
    pub session_id: String,
    #[serde(default)]
    pub turn_id: Option<String>,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub tool_results: Option<serde_json::Value>,
    #[serde(default)]
    pub timestamp_ms: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppendTurnResponseBody {
    pub success: bool,
    pub message_id: String,
    pub session_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CompleteTurnPayload {
    pub session_id: String,
    #[serde(default)]
    pub turn_id: Option<String>,
    pub assistant_response: String,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub timestamp_ms: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompleteTurnResponseBody {
    pub success: bool,
    pub session_id: String,
    pub total_turns: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SearchMemoryPayload {
    #[serde(default)]
    pub session_id: Option<String>,
    pub query: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemoryItemDto {
    pub node_id: String,
    pub label: String,
    pub excerpt: String,
    pub score: i64,
    pub channel: String,
    pub timestamp: i64,
    pub scope: String,
    #[serde(default)]
    pub relations: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchMemoryResponseBody {
    pub memories: Vec<MemoryItemDto>,
    pub provenance: serde_json::Value,
    pub token_count: usize,
    pub serialized_context: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TranscriptSearchQueryPayload {
    pub query: String,
    #[serde(default)]
    pub kinds: Option<Vec<String>>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TranscriptSearchMatchDto {
    pub id: String,
    pub kind: String,
    #[serde(rename = "sessionId", alias = "session_id", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub title: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TranscriptSearchResponseBody {
    pub matches: Vec<TranscriptSearchMatchDto>,
    pub total: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StoreMemoryPayload {
    #[serde(default)]
    pub session_id: Option<String>,
    pub label: String,
    pub content: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub relations: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoreMemoryResponseBody {
    pub success: bool,
    pub node_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LinkMemoryPayload {
    #[serde(rename = "sourceNodeId", alias = "source_node_id")]
    pub source_node_id: String,
    #[serde(rename = "targetNodeId", alias = "target_node_id")]
    pub target_node_id: String,
    #[serde(default)]
    pub relation: Option<String>,
    #[serde(default)]
    pub weight: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinkMemoryResponseBody {
    pub success: bool,
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UnlinkMemoryPayload {
    #[serde(rename = "sourceNodeId", alias = "source_node_id")]
    pub source_node_id: String,
    #[serde(rename = "targetNodeId", alias = "target_node_id")]
    pub target_node_id: String,
    #[serde(default)]
    pub relation: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnlinkMemoryResponseBody {
    pub success: bool,
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemStatusResponseBody {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub sqlite_healthy: bool,
    pub total_sessions: usize,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub active_models_count: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ToolFeedbackIdentityDto {
    #[serde(rename = "sessionId", alias = "session_id")]
    pub session_id: String,
    #[serde(rename = "turnId", alias = "turn_id")]
    pub turn_id: String,
    #[serde(rename = "toolUseId", alias = "tool_use_id")]
    pub tool_use_id: String,
    #[serde(default)]
    pub sequence: Option<u64>,
    #[serde(default)]
    pub timestamp: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ToolFeedbackToolDto {
    pub name: String,
    pub status: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ToolFeedbackOperationDto {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(rename = "commandName", alias = "command_name", default)]
    pub command_name: Option<String>,
    #[serde(rename = "affectedPaths", alias = "affected_paths", default)]
    pub affected_paths: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ToolFeedbackResultDto {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(rename = "isError", alias = "is_error", default)]
    pub is_error: bool,
    #[serde(rename = "durationMs", alias = "duration_ms", default)]
    pub duration_ms: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ToolExecutionFeedbackPayload {
    pub identity: ToolFeedbackIdentityDto,
    pub tool: ToolFeedbackToolDto,
    #[serde(default)]
    pub operation: ToolFeedbackOperationDto,
    #[serde(default)]
    pub result: ToolFeedbackResultDto,
    #[serde(rename = "payloadHash", alias = "payload_hash", default)]
    pub payload_hash: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolFeedbackResponseBody {
    pub success: bool,
    pub event_id: String,
    pub facts_ingested: usize,
    pub entities_linked: Vec<String>,
    pub is_duplicate: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ListSessionsPayload {
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(rename = "workspacePath", alias = "workspace_path", default)]
    pub workspace_path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionSummaryDto {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub title: String,
    #[serde(rename = "messageCount")]
    pub message_count: usize,
    #[serde(rename = "createdAtMs")]
    pub created_at_ms: i64,
    #[serde(rename = "updatedAtMs")]
    pub updated_at_ms: i64,
    #[serde(rename = "workspacePath")]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(rename = "systemPrompt", default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListSessionsResponseBody {
    pub sessions: Vec<SessionSummaryDto>,
    pub total: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LoadSessionPayload {
    #[serde(rename = "sessionId", alias = "session_id")]
    pub session_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SetSessionSystemPromptPayload {
    #[serde(rename = "sessionId", alias = "session_id")]
    pub session_id: String,
    #[serde(rename = "systemPrompt", alias = "system_prompt", default)]
    pub system_prompt: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionMessageDto {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionDetailDto {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub title: String,
    #[serde(rename = "createdAtMs")]
    pub created_at_ms: i64,
    #[serde(rename = "updatedAtMs")]
    pub updated_at_ms: i64,
    #[serde(rename = "workspacePath")]
    pub workspace_path: Option<String>,
    #[serde(rename = "systemPrompt", default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub messages: Vec<SessionMessageDto>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoadSessionResponseBody {
    pub session: SessionDetailDto,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ConsolidatePayload {
    #[serde(default, alias = "promotionWeightThreshold")]
    pub promotion_weight_threshold: Option<f64>,
    #[serde(default, alias = "pruningWeightThreshold")]
    pub pruning_weight_threshold: Option<f64>,
    #[serde(default, alias = "stalenessAgeThresholdSecs")]
    pub staleness_age_threshold_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConsolidationResponseBody {
    #[serde(rename = "actionsApplied")]
    pub actions_applied: usize,
    pub promoted: usize,
    pub merged: usize,
    pub archived: usize,
    pub pruned: usize,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ModelDescriptorDto {
    pub id: String,
    pub name: String,
    pub provider: String,
    #[serde(rename = "contextWindow")]
    pub context_window: usize,
    #[serde(rename = "maxOutputTokens")]
    pub max_output_tokens: usize,
    #[serde(rename = "supportsThinking")]
    pub supports_thinking: bool,
    #[serde(rename = "supportsTools")]
    pub supports_tools: bool,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListModelsResponseBody {
    pub models: Vec<ModelDescriptorDto>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ResolveModelPayload {
    pub query: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolveModelResponseBody {
    pub model: ModelDescriptorDto,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "stream_start")]
    Start {
        #[serde(rename = "streamId")]
        stream_id: String,
        #[serde(default)]
        metadata: serde_json::Value,
    },
    #[serde(rename = "stream_progress")]
    Progress {
        #[serde(rename = "streamId")]
        stream_id: String,
        sequence: u64,
        progress: f64,
        message: String,
        #[serde(default)]
        metadata: serde_json::Value,
    },
    #[serde(rename = "stream_chunk")]
    Chunk {
        #[serde(rename = "streamId")]
        stream_id: String,
        sequence: u64,
        content: String,
        #[serde(default)]
        metadata: serde_json::Value,
    },
    #[serde(rename = "stream_end")]
    End {
        #[serde(rename = "streamId")]
        stream_id: String,
        sequence: u64,
        #[serde(default)]
        metadata: serde_json::Value,
    },
    #[serde(rename = "stream_cancelled")]
    Cancelled {
        #[serde(rename = "streamId")]
        stream_id: String,
        sequence: u64,
        #[serde(default)]
        metadata: serde_json::Value,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ServerResponse {
    Response(VersionedResponse),
    Error(VersionedError),
    Event(VersionedEvent),
    Notification(VersionedNotification),
    Stream(StreamEvent),
    Legacy(LegacyResponse),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_request_parsing() {
        let json_data = r#"{"action": "ingest", "payload": "hello"}"#;
        let request: ClientRequest = serde_json::from_str(json_data).unwrap();
        match request {
            ClientRequest::Legacy(req) => {
                assert_eq!(req.action, "ingest");
                assert_eq!(req.payload, "hello");
            }
            _ => panic!("Expected legacy request"),
        }
    }

    #[test]
    fn test_versioned_request_parsing() {
        let json_data = r#"{"version": "1.0", "type": "Request", "id": 42, "action": "query", "body": "find me"}"#;
        let request: ClientRequest = serde_json::from_str(json_data).unwrap();
        match request {
            ClientRequest::Versioned(req) => {
                assert_eq!(req.version, "1.0");
                assert_eq!(req.msg_type, "Request");
                assert_eq!(req.id, 42);
                assert_eq!(req.action, "query");
                assert_eq!(req.body, "find me");
            }
            _ => panic!("Expected versioned request"),
        }
    }

    #[test]
    fn test_jsonrpc_request_parsing() {
        let json_data =
            r#"{"id": "req_01", "action": "v1/session/create", "payload": {"title": "Test"}}"#;
        let request: ClientRequest = serde_json::from_str(json_data).unwrap();
        match request {
            ClientRequest::JsonRpc(req) => {
                assert_eq!(req.id, "req_01");
                assert_eq!(req.action, "v1/session/create");
                assert_eq!(req.payload["title"], "Test");
            }
            _ => panic!("Expected jsonrpc request"),
        }
    }

    #[test]
    fn test_legacy_response_serialization() {
        let response = ServerResponse::Legacy(LegacyResponse {
            status: "ok".to_string(),
            message: "done".to_string(),
        });
        let json_str = serde_json::to_string(&response).unwrap();
        assert!(json_str.contains(r#""status":"ok""#));
        assert!(json_str.contains(r#""message":"done""#));
        assert!(!json_str.contains(r#""type""#));
    }

    #[test]
    fn test_versioned_response_serialization() {
        let response = ServerResponse::Response(VersionedResponse {
            version: "1.0".to_string(),
            msg_type: "Response".to_string(),
            id: 42,
            status: "success".to_string(),
            body: "result".to_string(),
        });
        let json_str = serde_json::to_string(&response).unwrap();
        assert!(json_str.contains(r#""version":"1.0""#));
        assert!(json_str.contains(r#""type":"Response""#));
        assert!(json_str.contains(r#""id":42"#));
        assert!(json_str.contains(r#""status":"success""#));
        assert!(json_str.contains(r#""body":"result""#));
    }

    #[test]
    fn test_versioned_error_serialization() {
        let err = ServerResponse::Error(VersionedError {
            version: "1.0".to_string(),
            msg_type: "Error".to_string(),
            id: 42,
            status: "error".to_string(),
            body: "something failed".to_string(),
        });
        let json_str = serde_json::to_string(&err).unwrap();
        assert!(json_str.contains(r#""type":"Error""#));
        assert!(json_str.contains(r#""status":"error""#));
        assert!(json_str.contains(r#""body":"something failed""#));
    }

    #[test]
    fn test_versioned_event_serialization() {
        let event = ServerResponse::Event(VersionedEvent {
            version: "1.0".to_string(),
            msg_type: "Event".to_string(),
            event_name: "epoch_rotated".to_string(),
            payload: serde_json::json!({ "epoch": 5 }),
        });
        let json_str = serde_json::to_string(&event).unwrap();
        assert!(json_str.contains(r#""type":"Event""#));
        assert!(json_str.contains(r#""event_name":"epoch_rotated""#));
        assert!(json_str.contains(r#""payload":{"epoch":5}"#));
    }

    #[test]
    fn test_versioned_notification_serialization() {
        let notif = ServerResponse::Notification(VersionedNotification {
            version: "1.0".to_string(),
            msg_type: "Notification".to_string(),
            notification_type: "sync_complete".to_string(),
            message: "duckdb sync completed".to_string(),
        });
        let json_str = serde_json::to_string(&notif).unwrap();
        assert!(json_str.contains(r#""type":"Notification""#));
        assert!(json_str.contains(r#""notification_type":"sync_complete""#));
        assert!(json_str.contains(r#""message":"duckdb sync completed""#));
    }

    #[test]
    fn test_stream_event_serialization() {
        let start = ServerResponse::Stream(StreamEvent::Start {
            stream_id: "test-stream".to_string(),
            metadata: serde_json::json!({ "model": "test-model" }),
        });
        let json_start = serde_json::to_string(&start).unwrap();
        assert!(json_start.contains(r#""type":"stream_start""#));
        assert!(json_start.contains(r#""streamId":"test-stream""#));
        assert!(json_start.contains(r#""metadata":{"model":"test-model"}"#));

        let chunk = ServerResponse::Stream(StreamEvent::Chunk {
            stream_id: "test-stream".to_string(),
            sequence: 42,
            content: "hello world".to_string(),
            metadata: serde_json::json!({}),
        });
        let json_chunk = serde_json::to_string(&chunk).unwrap();
        assert!(json_chunk.contains(r#""type":"stream_chunk""#));
        assert!(json_chunk.contains(r#""streamId":"test-stream""#));
        assert!(json_chunk.contains(r#""sequence":42"#));
        assert!(json_chunk.contains(r#""content":"hello world""#));
    }

    #[test]
    fn test_schema_conformance() {
        use std::path::Path;
        let schema_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../protocol/uds_ipc.schema.json");
        let schema_content =
            std::fs::read_to_string(schema_path).expect("Failed to read schema file");
        let schema_json: serde_json::Value =
            serde_json::from_str(&schema_content).expect("Failed to parse schema JSON");
        let defs = schema_json.get("$defs").expect("Missing $defs in schema");

        // Compile validators
        let req_schema = serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "$ref": "#/$defs/Request",
            "$defs": defs
        });
        let req_validator = jsonschema::JSONSchema::compile(&req_schema).unwrap();

        let resp_schema = serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "$ref": "#/$defs/Response",
            "$defs": defs
        });
        let resp_validator = jsonschema::JSONSchema::compile(&resp_schema).unwrap();

        let stream_schema = serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "$ref": "#/$defs/StreamEvent",
            "$defs": defs
        });
        let stream_validator = jsonschema::JSONSchema::compile(&stream_schema).unwrap();

        // 1. Positive Tests (Serialize -> Validate -> Deserialize -> Match)

        // A. Legacy Request
        let legacy_req = ClientRequest::Legacy(LegacyRequest {
            action: "query".to_string(),
            payload: "sqlite".to_string(),
        });
        let json_legacy_req = serde_json::to_value(&legacy_req).unwrap();
        assert!(req_validator.validate(&json_legacy_req).is_ok());
        let roundtrip: ClientRequest = serde_json::from_value(json_legacy_req.clone()).unwrap();
        assert_eq!(
            serde_json::to_string(&legacy_req).unwrap(),
            serde_json::to_string(&roundtrip).unwrap()
        );

        // B. Versioned Request
        let versioned_req = ClientRequest::Versioned(VersionedRequest {
            version: "1.0".to_string(),
            msg_type: "Request".to_string(),
            id: 101,
            action: "ingest".to_string(),
            body: "postgres config".to_string(),
            workspace_context: vec![],
        });
        let json_versioned_req = serde_json::to_value(&versioned_req).unwrap();
        assert!(req_validator.validate(&json_versioned_req).is_ok());
        let roundtrip: ClientRequest = serde_json::from_value(json_versioned_req.clone()).unwrap();
        assert_eq!(
            serde_json::to_string(&versioned_req).unwrap(),
            serde_json::to_string(&roundtrip).unwrap()
        );

        // C. Legacy Response
        let legacy_resp = ServerResponse::Legacy(LegacyResponse {
            status: "ok".to_string(),
            message: "Success".to_string(),
        });
        let json_legacy_resp = serde_json::to_value(&legacy_resp).unwrap();
        assert!(resp_validator.validate(&json_legacy_resp).is_ok());

        // D. Versioned Response
        let versioned_resp = ServerResponse::Response(VersionedResponse {
            version: "1.0".to_string(),
            msg_type: "Response".to_string(),
            id: 201,
            status: "success".to_string(),
            body: "Found 1 matches".to_string(),
        });
        let json_versioned_resp = serde_json::to_value(&versioned_resp).unwrap();
        assert!(resp_validator.validate(&json_versioned_resp).is_ok());

        // E. Versioned Error
        let versioned_err = ServerResponse::Error(VersionedError {
            version: "1.0".to_string(),
            msg_type: "Error".to_string(),
            id: 301,
            status: "error".to_string(),
            body: "Fatal error".to_string(),
        });
        let json_versioned_err = serde_json::to_value(&versioned_err).unwrap();
        assert!(resp_validator.validate(&json_versioned_err).is_ok());

        // F. Versioned Notification
        let versioned_notif = ServerResponse::Notification(VersionedNotification {
            version: "1.0".to_string(),
            msg_type: "Notification".to_string(),
            notification_type: "sync".to_string(),
            message: "complete".to_string(),
        });
        let json_versioned_notif = serde_json::to_value(&versioned_notif).unwrap();
        assert!(resp_validator.validate(&json_versioned_notif).is_ok());

        // G. Versioned Event
        let versioned_event = ServerResponse::Event(VersionedEvent {
            version: "1.0".to_string(),
            msg_type: "Event".to_string(),
            event_name: "rotated".to_string(),
            payload: serde_json::json!({"epoch": 4}),
        });
        let json_versioned_event = serde_json::to_value(&versioned_event).unwrap();
        assert!(resp_validator.validate(&json_versioned_event).is_ok());

        // H. Stream Event (Start)
        let stream_start = StreamEvent::Start {
            stream_id: "stream-99".to_string(),
            metadata: serde_json::json!({"timing": 0.5}),
        };
        let json_stream_start = serde_json::to_value(&stream_start).unwrap();
        assert!(stream_validator.validate(&json_stream_start).is_ok());

        // I. Stream Event (Progress)
        let stream_progress = StreamEvent::Progress {
            stream_id: "stream-99".to_string(),
            sequence: 1,
            progress: 0.75,
            message: "Syncing".to_string(),
            metadata: serde_json::json!({}),
        };
        let json_stream_progress = serde_json::to_value(&stream_progress).unwrap();
        assert!(stream_validator.validate(&json_stream_progress).is_ok());

        // J. Stream Event (Chunk)
        let stream_chunk = StreamEvent::Chunk {
            stream_id: "stream-99".to_string(),
            sequence: 2,
            content: "payload".to_string(),
            metadata: serde_json::json!({}),
        };
        let json_stream_chunk = serde_json::to_value(&stream_chunk).unwrap();
        assert!(stream_validator.validate(&json_stream_chunk).is_ok());

        // K. Stream Event (End)
        let stream_end = StreamEvent::End {
            stream_id: "stream-99".to_string(),
            sequence: 3,
            metadata: serde_json::json!({}),
        };
        let json_stream_end = serde_json::to_value(&stream_end).unwrap();
        assert!(stream_validator.validate(&json_stream_end).is_ok());

        // L. Stream Event (Cancelled)
        let stream_cancelled = StreamEvent::Cancelled {
            stream_id: "stream-99".to_string(),
            sequence: 3,
            metadata: serde_json::json!({}),
        };
        let json_stream_cancelled = serde_json::to_value(&stream_cancelled).unwrap();
        assert!(stream_validator.validate(&json_stream_cancelled).is_ok());

        // 2. Negative Validation Tests

        // A. Missing required field (e.g. missing action on versioned request)
        let mut bad_versioned_req = json_versioned_req.clone();
        bad_versioned_req.as_object_mut().unwrap().remove("action");
        assert!(req_validator.validate(&bad_versioned_req).is_err());

        // B. Incorrect field type (e.g. sequence number as string instead of integer)
        let mut bad_stream_progress = json_stream_progress.clone();
        bad_stream_progress
            .as_object_mut()
            .unwrap()
            .insert("sequence".to_string(), serde_json::json!("1"));
        assert!(stream_validator.validate(&bad_stream_progress).is_err());

        // C. Out-of-bounds float progress (> 1.0)
        let mut bad_stream_progress_val = json_stream_progress.clone();
        bad_stream_progress_val
            .as_object_mut()
            .unwrap()
            .insert("progress".to_string(), serde_json::json!(1.5));
        assert!(stream_validator.validate(&bad_stream_progress_val).is_err());

        // D. Extraneous property (additionalProperties: false)
        let mut bad_versioned_resp = json_versioned_resp.clone();
        bad_versioned_resp.as_object_mut().unwrap().insert(
            "unexpected_field".to_string(),
            serde_json::json!("not-allowed"),
        );
        assert!(resp_validator.validate(&bad_versioned_resp).is_err());

        // E. Version is not "1.0"
        let mut bad_versioned_err = json_versioned_err.clone();
        bad_versioned_err
            .as_object_mut()
            .unwrap()
            .insert("version".to_string(), serde_json::json!("2.0"));
        assert!(resp_validator.validate(&bad_versioned_err).is_err());
    }

    // --- RFC-007 workspace_context serialization tests ---

    #[test]
    fn test_workspace_context_absent_when_empty() {
        // When workspace_context is empty (not supplied), it MUST NOT appear in
        // the serialised JSON so old daemons receive a clean wire frame.
        let req = VersionedRequest {
            version: "1.0".to_string(),
            msg_type: "Request".to_string(),
            id: 1,
            action: "query".to_string(),
            body: "hello".to_string(),
            workspace_context: vec![],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(
            !json.contains("workspace_context"),
            "workspace_context must not appear in JSON when empty; got: {}",
            json
        );
    }

    #[test]
    fn test_workspace_context_serializes_when_present() {
        // When workspace_context is non-empty, it MUST appear as a top-level
        // sibling array of strings.
        let req = VersionedRequest {
            version: "1.0".to_string(),
            msg_type: "Request".to_string(),
            id: 1,
            action: "query".to_string(),
            body: "hello".to_string(),
            workspace_context: vec!["node-aaa".to_string(), "node-bbb".to_string()],
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let ctx = parsed["workspace_context"]
            .as_array()
            .expect("workspace_context must be an array");
        assert_eq!(ctx.len(), 2);
        assert_eq!(ctx[0].as_str().unwrap(), "node-aaa");
        assert_eq!(ctx[1].as_str().unwrap(), "node-bbb");
    }

    #[test]
    fn test_session_summary_dto_pinned_archived_serde() {
        // 1. Deserializing full JSON preserves true values
        let json = r#"{"sessionId":"s1","title":"T1","messageCount":2,"createdAtMs":1000,"updatedAtMs":2000,"workspacePath":null,"pinned":true,"archived":false}"#;
        let dto: SessionSummaryDto = serde_json::from_str(json).unwrap();
        assert_eq!(dto.session_id, "s1");
        assert!(dto.pinned);
        assert!(!dto.archived);

        // 2. Deserializing legacy JSON without pinned/archived defaults safely to false
        let legacy = r#"{"sessionId":"s2","title":"T2","messageCount":1,"createdAtMs":1000,"updatedAtMs":2000,"workspacePath":null}"#;
        let dto_legacy: SessionSummaryDto = serde_json::from_str(legacy).unwrap();
        assert_eq!(dto_legacy.session_id, "s2");
        assert!(!dto_legacy.pinned);
        assert!(!dto_legacy.archived);

        // 3. Serialization includes pinned and archived
        let serialized = serde_json::to_string(&dto).unwrap();
        assert!(serialized.contains(r#""pinned":true"#));
        assert!(serialized.contains(r#""archived":false"#));
    }
}
