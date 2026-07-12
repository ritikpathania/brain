use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio_util::sync::CancellationToken;
use brain_application::{BrainApplication, ExecutionContext, ApplicationEvent, ApplicationEventSink};
use crate::protocol::{AcpRequest, AcpResponse, AcpNotification};
use crate::registry::CapabilityExposureRegistry;
use crate::mapping::{AcpErrorMapper, AcpEventMapper};

struct AcpEventSink {
    session_id: String,
    on_notification: Arc<dyn Fn(AcpNotification) + Send + Sync>,
}

impl ApplicationEventSink for AcpEventSink {
    fn emit(&self, event: ApplicationEvent) {
        let notif = AcpEventMapper::map(event, &self.session_id);
        (self.on_notification)(notif);
    }
}

/// The ACP Protocol Adapter managing JSON-RPC 2.0 requests over stdio,
/// mapping session prompts and cancellations to core capability executions.
pub struct AcpAdapter {
    app: Arc<BrainApplication>,
    registry: CapabilityExposureRegistry,
    on_notification: Arc<dyn Fn(AcpNotification) + Send + Sync>,
    active_runs: Arc<RwLock<HashMap<String, CancellationToken>>>,
}

impl AcpAdapter {
    /// Create a new AcpAdapter instance.
    pub fn new(
        app: Arc<BrainApplication>,
        on_notification: Arc<dyn Fn(AcpNotification) + Send + Sync>,
    ) -> Self {
        Self {
            app,
            registry: crate::registry::create_registry(),
            on_notification,
            active_runs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Process a single incoming JSON-RPC request.
    pub async fn handle_request(&self, req: AcpRequest) -> Option<AcpResponse> {
        let req_id = req.id.clone().unwrap_or(serde_json::Value::Null);

        // 1. Protocol Conformance checks
        if req.jsonrpc != "2.0" {
            return Some(AcpResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(crate::protocol::AcpError {
                    code: -32600, // Invalid Request
                    message: "Invalid JSON-RPC protocol version. Expected '2.0'.".to_string(),
                    data: None,
                }),
                id: req_id,
            });
        }

        match req.method.as_str() {
            "initialize" => {
                let caps_list: Vec<&str> = self.registry.list().iter().map(|c| c.name()).collect();
                let res = serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "applicationInterface": brain_application::BrainApplication::INTERFACE_VERSION,
                    "capabilities": {
                        "session": {},
                        "list": caps_list
                    },
                    "serverInfo": {
                        "name": "brain-acp-server",
                        "version": "0.1.0"
                    }
                });
                Some(AcpResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(res),
                    error: None,
                    id: req_id,
                })
            }
            "session/new" => {
                let params = req.params.unwrap_or(serde_json::json!({}));
                let session_id = params.get("sessionId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                let res = serde_json::json!({
                    "sessionId": session_id
                });
                Some(AcpResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(res),
                    error: None,
                    id: req_id,
                })
            }
            "session/prompt" => {
                let params = match req.params {
                    Some(p) => p,
                    None => {
                        return Some(AcpResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(crate::protocol::AcpError {
                                code: -32602, // Invalid Params
                                message: "Missing params object in session/prompt".to_string(),
                                data: None,
                            }),
                            id: req_id,
                        });
                    }
                };

                let session_id = match params.get("sessionId").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Some(AcpResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(crate::protocol::AcpError {
                                code: -32602,
                                message: "Missing sessionId in session/prompt".to_string(),
                                data: None,
                            }),
                            id: req_id,
                        });
                    }
                };

                // Extract capability name, defaulting to "ingest"
                let capability_name = params.get("capability")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ingest");

                let cap = match self.registry.get(capability_name) {
                    Some(c) => c,
                    None => {
                        return Some(AcpResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(crate::protocol::AcpError {
                                code: -32601, // Method/Capability not found
                                message: format!("Capability '{}' is not registered.", capability_name),
                                data: None,
                            }),
                            id: req_id,
                        });
                    }
                };

                let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

                // Establish execution token cancellation
                let cancel_token = CancellationToken::new();
                {
                    let mut runs = self.active_runs.write().unwrap();
                    runs.insert(session_id.clone(), cancel_token.clone());
                }

                // Configure ExecutionContext
                let context = ExecutionContext::default()
                    .with_cancellation(cancel_token)
                    .with_event_sink(AcpEventSink {
                        session_id: session_id.clone(),
                        on_notification: self.on_notification.clone(),
                    });

                let execute_result = cap.execute_erased(&self.app, arguments, &context).await;

                // Execution finished: cleanup token
                {
                    let mut runs = self.active_runs.write().unwrap();
                    runs.remove(&session_id);
                }

                match execute_result {
                    Ok(res) => {
                        let acp_res = serde_json::json!({
                            "content": serde_json::to_string_pretty(&res).unwrap_or_default()
                        });
                        Some(AcpResponse {
                            jsonrpc: "2.0".to_string(),
                            result: Some(acp_res),
                            error: None,
                            id: req_id,
                        })
                    }
                    Err(app_err) => {
                        let acp_err = AcpErrorMapper::map(app_err);
                        Some(AcpResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(acp_err),
                            id: req_id,
                        })
                    }
                }
            }
            _ => Some(AcpResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(crate::protocol::AcpError {
                    code: -32601, // Method not found
                    message: format!("Unknown JSON-RPC method: '{}'", req.method),
                    data: None,
                }),
                id: req_id,
            }),
        }
    }

    /// Process an incoming JSON-RPC notification (no response output required).
    pub fn handle_notification(&self, method: &str, params: Option<serde_json::Value>) {
        if method == "session/cancel" {
            if let Some(params) = params {
                if let Some(session_id) = params.get("sessionId").and_then(|v| v.as_str()) {
                    let runs = self.active_runs.read().unwrap();
                    if let Some(token) = runs.get(session_id) {
                        token.cancel();
                    }
                }
            }
        }
    }
}
