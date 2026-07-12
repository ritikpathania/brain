use serde::{Deserialize, Serialize};

/// Standard ACP JSON-RPC 2.0 request envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcpRequest {
    /// Protocol version, must be "2.0".
    pub jsonrpc: String,
    /// Targeted method name.
    pub method: String,
    /// Method parameters payload.
    pub params: Option<serde_json::Value>,
    /// Request identifier.
    pub id: Option<serde_json::Value>,
}

/// Standard ACP JSON-RPC 2.0 response envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcpResponse {
    /// Protocol version, must be "2.0".
    pub jsonrpc: String,
    /// Result payload if successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error payload if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AcpError>,
    /// Request identifier matched from request.
    pub id: serde_json::Value,
}

/// Standard ACP JSON-RPC 2.0 error block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcpError {
    /// Error code classifications.
    pub code: i32,
    /// Description message.
    pub message: String,
    /// Structured metadata details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Standard ACP JSON-RPC 2.0 notification frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcpNotification {
    /// Protocol version, must be "2.0".
    pub jsonrpc: String,
    /// Notification method name.
    pub method: String,
    /// Notification parameters payload.
    pub params: serde_json::Value,
}
