use brain_application::{ApplicationError, ApplicationEvent};
use crate::protocol::{JsonRpcError, JsonRpcNotification};

/// Explicit error mapping layer translating ApplicationErrors to JSON-RPC 2.0 errors.
pub struct McpErrorMapper;

impl McpErrorMapper {
    /// Maps a semantic ApplicationError into a structured JsonRpcError.
    pub fn map(err: ApplicationError) -> JsonRpcError {
        match err {
            ApplicationError::Validation(msg) => JsonRpcError {
                code: -32602, // Invalid Params
                message: format!("Validation failed: {}", msg),
                data: None,
            },
            ApplicationError::Conflict(msg) => JsonRpcError {
                code: -32001, // Custom Server Error Conflict
                message: format!("Conflict: {}", msg),
                data: None,
            },
            ApplicationError::Unavailable(msg) => JsonRpcError {
                code: -32002, // Custom Server Error Unavailable
                message: format!("Unavailable: {}", msg),
                data: None,
            },
            ApplicationError::Cancelled(msg) => JsonRpcError {
                code: -32003, // Custom Server Error Cancelled
                message: format!("Operation cancelled: {}", msg),
                data: None,
            },
            ApplicationError::Timeout(msg) => JsonRpcError {
                code: -32004, // Custom Server Error Timeout
                message: format!("Operation timed out: {}", msg),
                data: None,
            },
            ApplicationError::Internal(msg) => JsonRpcError {
                code: -32603, // Internal Error
                message: format!("Internal system error: {}", msg),
                data: None,
            },
        }
    }
}

/// Explicit event mapping layer translating ApplicationEvents to standard MCP notifications.
pub struct McpEventMapper;

impl McpEventMapper {
    /// Translates a semantic ApplicationEvent into a JSON-RPC notification.
    pub fn map(event: ApplicationEvent, request_token: &str) -> JsonRpcNotification {
        match event {
            ApplicationEvent::Progress(p) => {
                let params = serde_json::json!({
                    "progressToken": request_token,
                    "step": p.step,
                    "total": p.total_steps,
                    "message": p.message,
                });
                JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "$/progress".to_string(),
                    params,
                }
            }
            ApplicationEvent::Warning(msg) => {
                let params = serde_json::json!({
                    "level": "warning",
                    "message": msg,
                });
                JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "notifications/message".to_string(), // standard MCP message notification
                    params,
                }
            }
            ApplicationEvent::Diagnostic(msg) => {
                let params = serde_json::json!({
                    "level": "debug",
                    "message": msg,
                });
                JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "notifications/message".to_string(),
                    params,
                }
            }
            ApplicationEvent::Completed(msg) => {
                let params = serde_json::json!({
                    "level": "info",
                    "message": msg,
                });
                JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "notifications/message".to_string(),
                    params,
                }
            }
        }
    }
}
