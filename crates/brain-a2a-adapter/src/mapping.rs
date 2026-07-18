use crate::protocol::{A2aError, A2aNotification};
use brain_application::{ApplicationError, ApplicationEvent};

/// Explicit error mapper translating ApplicationError into A2aError.
pub struct A2aErrorMapper;

impl A2aErrorMapper {
    /// Maps a semantic ApplicationError into a JSON-RPC 2.0 A2aError.
    pub fn map(err: ApplicationError) -> A2aError {
        match err {
            ApplicationError::Validation(msg) => A2aError {
                code: -32602, // Invalid Params
                message: format!("A2A validation failed: {}", msg),
                data: None,
            },
            ApplicationError::Conflict(msg) => A2aError {
                code: -32001,
                message: format!("A2A conflict: {}", msg),
                data: None,
            },
            ApplicationError::Unavailable(msg) => A2aError {
                code: -32002,
                message: format!("A2A service unavailable: {}", msg),
                data: None,
            },
            ApplicationError::Cancelled(msg) => A2aError {
                code: -32003,
                message: format!("A2A task cancelled: {}", msg),
                data: None,
            },
            ApplicationError::Timeout(msg) => A2aError {
                code: -32004,
                message: format!("A2A operation timeout: {}", msg),
                data: None,
            },
            ApplicationError::Internal(msg) => A2aError {
                code: -32603, // Internal Error
                message: format!("A2A internal error: {}", msg),
                data: None,
            },
        }
    }
}

/// Explicit event mapper translating ApplicationEvent into A2aNotification.
pub struct A2aEventMapper;

impl A2aEventMapper {
    /// Maps a semantic ApplicationEvent to an A2A agent/update notification.
    pub fn map(event: ApplicationEvent, session_id: &str) -> A2aNotification {
        match event {
            ApplicationEvent::Progress(p) => {
                let params = serde_json::json!({
                    "sessionId": session_id,
                    "type": "progress",
                    "step": p.step,
                    "total": p.total_steps,
                    "message": p.message,
                });
                A2aNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "agent/update".to_string(),
                    params,
                }
            }
            ApplicationEvent::Warning(msg) => {
                let params = serde_json::json!({
                    "sessionId": session_id,
                    "type": "warning",
                    "message": msg,
                });
                A2aNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "agent/update".to_string(),
                    params,
                }
            }
            ApplicationEvent::Diagnostic(msg) => {
                let params = serde_json::json!({
                    "sessionId": session_id,
                    "type": "diagnostic",
                    "message": msg,
                });
                A2aNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "agent/update".to_string(),
                    params,
                }
            }
            ApplicationEvent::Completed(msg) => {
                let params = serde_json::json!({
                    "sessionId": session_id,
                    "type": "completed",
                    "message": msg,
                });
                A2aNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "agent/update".to_string(),
                    params,
                }
            }
        }
    }
}
