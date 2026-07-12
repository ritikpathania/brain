use serde::{Deserialize, Serialize};

/// Standard JSON-RPC 2.0 request envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcRequest {
    /// Protocol version, must be "2.0".
    pub jsonrpc: String,
    /// Targeted method name.
    pub method: String,
    /// Method call parameters.
    pub params: Option<serde_json::Value>,
    /// Request identifier (null, number, or string).
    pub id: Option<serde_json::Value>,
}

/// Standard JSON-RPC 2.0 response envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcResponse {
    /// Protocol version, must be "2.0".
    pub jsonrpc: String,
    /// Result payload if successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error envelope if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// Request identifier matched from request.
    pub id: serde_json::Value,
}

/// Standard JSON-RPC 2.0 error block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcError {
    /// Error code classifications.
    pub code: i32,
    /// Description message.
    pub message: String,
    /// Structured metadata details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Standard JSON-RPC 2.0 notification frame (e.g. progress updates).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcNotification {
    /// Protocol version, must be "2.0".
    pub jsonrpc: String,
    /// Notification method name.
    pub method: String,
    /// Notification payload parameters.
    pub params: serde_json::Value,
}
