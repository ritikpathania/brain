use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio_util::sync::CancellationToken;
use brain_application::{BrainApplication, ExecutionContext, ApplicationEvent, ApplicationEventSink};
use crate::protocol::{A2aRequest, A2aResponse, A2aNotification};
use crate::registry::CapabilityRegistry;
use crate::mapping::{A2aErrorMapper, A2aEventMapper};

struct A2aEventSink {
    session_id: String,
    on_notification: Arc<dyn Fn(A2aNotification) + Send + Sync>,
}

impl ApplicationEventSink for A2aEventSink {
    fn emit(&self, event: ApplicationEvent) {
        let notif = A2aEventMapper::map(event, &self.session_id);
        (self.on_notification)(notif);
    }
}

/// The A2A Protocol Adapter managing message passing between agents,
/// supporting cancellation tracking and capability dispatching.
pub struct A2aAdapter {
    app: Arc<BrainApplication>,
    registry: CapabilityRegistry,
    on_notification: Arc<dyn Fn(A2aNotification) + Send + Sync>,
    active_runs: Arc<RwLock<HashMap<String, CancellationToken>>>,
}

impl A2aAdapter {
    /// Create a new A2aAdapter instance.
    pub fn new(
        app: Arc<BrainApplication>,
        on_notification: Arc<dyn Fn(A2aNotification) + Send + Sync>,
    ) -> Self {
        Self {
            app,
            registry: crate::registry::create_registry(),
            on_notification,
            active_runs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Process a single incoming JSON-RPC request.
    pub async fn handle_request(&self, req: A2aRequest) -> Option<A2aResponse> {
        let req_id = req.id.clone().unwrap_or(serde_json::Value::Null);

        if req.jsonrpc != "2.0" {
            return Some(A2aResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(crate::protocol::A2aError {
                    code: -32600, // Invalid Request
                    message: "Invalid JSON-RPC protocol version. Expected '2.0'.".to_string(),
                    data: None,
                }),
                id: req_id,
            });
        }

        match req.method.as_str() {
            "handshake" => {
                let res = serde_json::json!({
                    "version": "1.0.0",
                    "applicationInterface": brain_application::BrainApplication::INTERFACE_VERSION,
                    "capabilities": self.registry.list().iter().map(|c| c.name()).collect::<Vec<_>>(),
                });
                Some(A2aResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(res),
                    error: None,
                    id: req_id,
                })
            }
            "agent/message" => {
                let params = match req.params {
                    Some(p) => p,
                    None => {
                        return Some(A2aResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(crate::protocol::A2aError {
                                code: -32602,
                                message: "Missing parameters object in agent/message".to_string(),
                                data: None,
                            }),
                            id: req_id,
                        });
                    }
                };

                let session_id = match params.get("sessionId").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Some(A2aResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(crate::protocol::A2aError {
                                code: -32602,
                                message: "Missing sessionId in agent/message".to_string(),
                                data: None,
                            }),
                            id: req_id,
                        });
                    }
                };

                let capability_name = match params.get("capability").and_then(|v| v.as_str()) {
                    Some(c) => c,
                    None => {
                        return Some(A2aResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(crate::protocol::A2aError {
                                code: -32602,
                                message: "Missing capability name in agent/message".to_string(),
                                data: None,
                            }),
                            id: req_id,
                        });
                    }
                };

                let cap = match self.registry.get(capability_name) {
                    Some(c) => c,
                    None => {
                        return Some(A2aResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(crate::protocol::A2aError {
                                code: -32601, // Capability not found
                                message: format!("Capability '{}' not found.", capability_name),
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
                    .with_event_sink(A2aEventSink {
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
                    Ok(res) => Some(A2aResponse {
                        jsonrpc: "2.0".to_string(),
                        result: Some(res),
                        error: None,
                        id: req_id,
                    }),
                    Err(app_err) => {
                        let a2a_err = A2aErrorMapper::map(app_err);
                        Some(A2aResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(a2a_err),
                            id: req_id,
                        })
                    }
                }
            }
            _ => Some(A2aResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(crate::protocol::A2aError {
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
        if method == "agent/cancel" {
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
