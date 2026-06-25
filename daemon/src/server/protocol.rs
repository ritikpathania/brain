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
    pub body: String,
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
#[serde(untagged)]
pub enum ServerResponse {
    Response(VersionedResponse),
    Error(VersionedError),
    Event(VersionedEvent),
    Notification(VersionedNotification),
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
}
