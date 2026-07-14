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
#[serde(untagged)]
pub enum ClientRequest {
    Versioned(VersionedRequest),
    Legacy(LegacyRequest),
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
pub struct LegacyResponse {
    pub status: String,
    pub message: String,
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
            ClientRequest::Versioned(_) => panic!("Expected legacy request"),
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
            ClientRequest::Legacy(_) => panic!("Expected versioned request"),
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
        let schema_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../protocol/uds_ipc.schema.json");
        let schema_content = std::fs::read_to_string(schema_path).expect("Failed to read schema file");
        let schema_json: serde_json::Value = serde_json::from_str(&schema_content).expect("Failed to parse schema JSON");
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
        assert_eq!(serde_json::to_string(&legacy_req).unwrap(), serde_json::to_string(&roundtrip).unwrap());

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
        assert_eq!(serde_json::to_string(&versioned_req).unwrap(), serde_json::to_string(&roundtrip).unwrap());


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
        bad_stream_progress.as_object_mut().unwrap().insert("sequence".to_string(), serde_json::json!("1"));
        assert!(stream_validator.validate(&bad_stream_progress).is_err());

        // C. Out-of-bounds float progress (> 1.0)
        let mut bad_stream_progress_val = json_stream_progress.clone();
        bad_stream_progress_val.as_object_mut().unwrap().insert("progress".to_string(), serde_json::json!(1.5));
        assert!(stream_validator.validate(&bad_stream_progress_val).is_err());

        // D. Extraneous property (additionalProperties: false)
        let mut bad_versioned_resp = json_versioned_resp.clone();
        bad_versioned_resp.as_object_mut().unwrap().insert("unexpected_field".to_string(), serde_json::json!("not-allowed"));
        assert!(resp_validator.validate(&bad_versioned_resp).is_err());

        // E. Version is not "1.0"
        let mut bad_versioned_err = json_versioned_err.clone();
        bad_versioned_err.as_object_mut().unwrap().insert("version".to_string(), serde_json::json!("2.0"));
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
        let ctx = parsed["workspace_context"].as_array().expect("workspace_context must be an array");
        assert_eq!(ctx.len(), 2);
        assert_eq!(ctx[0].as_str().unwrap(), "node-aaa");
        assert_eq!(ctx[1].as_str().unwrap(), "node-bbb");
    }
}
