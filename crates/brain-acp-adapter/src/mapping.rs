use crate::protocol::{AcpError, AcpNotification};
use brain_application::{ApplicationError, ApplicationEvent};

/// Explicit error mapper translating ApplicationError into AcpError.
pub struct AcpErrorMapper;

impl AcpErrorMapper {
    /// Maps a semantic ApplicationError into a JSON-RPC 2.0 AcpError.
    pub fn map(err: ApplicationError) -> AcpError {
        match err {
            ApplicationError::Validation(msg) => AcpError {
                code: -32602, // Invalid Params
                message: format!("Validation failed: {}", msg),
                data: None,
            },
            ApplicationError::Conflict(msg) => AcpError {
                code: -32001, // Conflict
                message: format!("Conflict: {}", msg),
                data: None,
            },
            ApplicationError::Unavailable(msg) => AcpError {
                code: -32002, // Service Unavailable
                message: format!("Service unavailable: {}", msg),
                data: None,
            },
            ApplicationError::Cancelled(msg) => AcpError {
                code: -32003, // Cancelled
                message: format!("Operation cancelled: {}", msg),
                data: None,
            },
            ApplicationError::Timeout(msg) => AcpError {
                code: -32004, // Timeout
                message: format!("Operation timed out: {}", msg),
                data: None,
            },
            ApplicationError::Internal(msg) => AcpError {
                code: -32603, // Internal Error
                message: format!("Internal system error: {}", msg),
                data: None,
            },
        }
    }
}

/// Explicit event mapper translating ApplicationEvent into AcpNotification.
pub struct AcpEventMapper;

impl AcpEventMapper {
    /// Maps a semantic ApplicationEvent to an ACP session/update notification.
    pub fn map(event: ApplicationEvent, session_id: &str) -> AcpNotification {
        match event {
            ApplicationEvent::Progress(p) => {
                let params = serde_json::json!({
                    "sessionId": session_id,
                    "updateType": "progress",
                    "step": p.step,
                    "total": p.total_steps,
                    "message": p.message,
                });
                AcpNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params,
                }
            }
            ApplicationEvent::Warning(msg) => {
                let params = serde_json::json!({
                    "sessionId": session_id,
                    "updateType": "warning",
                    "message": msg,
                });
                AcpNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params,
                }
            }
            ApplicationEvent::Diagnostic(msg) => {
                let params = serde_json::json!({
                    "sessionId": session_id,
                    "updateType": "diagnostic",
                    "message": msg,
                });
                AcpNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params,
                }
            }
            ApplicationEvent::Completed(msg) => {
                let params = serde_json::json!({
                    "sessionId": session_id,
                    "updateType": "completed",
                    "message": msg,
                });
                AcpNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params,
                }
            }
        }
    }
}
