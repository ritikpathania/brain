use crate::mapping::{McpErrorMapper, McpEventMapper};
use crate::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::registry::CapabilityRegistry;
use brain_application::{
    ApplicationEvent, ApplicationEventSink, BrainApplication, ExecutionContext,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Stdio progress event dispatcher.
struct StdioEventSink {
    request_token: String,
    on_notification: Arc<dyn Fn(JsonRpcNotification) + Send + Sync>,
}

impl ApplicationEventSink for StdioEventSink {
    fn emit(&self, event: ApplicationEvent) {
        let notif = McpEventMapper::map(event, &self.request_token);
        (self.on_notification)(notif);
    }
}

/// The MCP Protocol Adapter class translating standard JSON-RPC 2.0 messages
/// into strongly typed Brain capability operations.
pub struct McpAdapter {
    app: Arc<BrainApplication>,
    registry: CapabilityRegistry,
    on_notification: Arc<dyn Fn(JsonRpcNotification) + Send + Sync>,
}

impl McpAdapter {
    /// Create a new McpAdapter instance.
    pub fn new(
        app: Arc<BrainApplication>,
        on_notification: Arc<dyn Fn(JsonRpcNotification) + Send + Sync>,
    ) -> Self {
        Self {
            app,
            registry: crate::registry::create_registry(),
            on_notification,
        }
    }

    /// Process a single incoming JSON-RPC 2.0 request.
    pub async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let req_id = req.id.unwrap_or(serde_json::Value::Null);

        // 1. Protocol Conformance checks
        if req.jsonrpc != "2.0" {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(crate::protocol::JsonRpcError {
                    code: -32600, // Invalid Request
                    message: "Invalid JSON-RPC protocol version. Expected '2.0'.".to_string(),
                    data: None,
                }),
                id: req_id,
            };
        }

        match req.method.as_str() {
            "initialize" => {
                let caps_list: Vec<&str> = self.registry.list().iter().map(|c| c.name()).collect();
                let res = serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "applicationInterface": brain_application::BrainApplication::INTERFACE_VERSION,
                    "capabilities": {
                        "tools": {},
                        "list": caps_list
                    },
                    "serverInfo": {
                        "name": "brain-mcp-server",
                        "version": "0.1.0"
                    }
                });
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(res),
                    error: None,
                    id: req_id,
                }
            }
            "tools/list" => {
                let mcp_tools: Vec<serde_json::Value> = self
                    .registry
                    .list()
                    .iter()
                    .map(|cap| {
                        serde_json::json!({
                            "name": format!("brain_{}", cap.name()),
                            "description": cap.description(),
                            "inputSchema": cap.input_schema()
                        })
                    })
                    .collect();

                let res = serde_json::json!({
                    "tools": mcp_tools
                });

                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(res),
                    error: None,
                    id: req_id,
                }
            }
            "tools/call" => {
                // Decode tool parameters
                let params = match req.params {
                    Some(p) => p,
                    None => {
                        return JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(crate::protocol::JsonRpcError {
                                code: -32602, // Invalid Params
                                message: "Missing params object in tools/call".to_string(),
                                data: None,
                            }),
                            id: req_id,
                        };
                    }
                };

                let tool_name = match params.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(crate::protocol::JsonRpcError {
                                code: -32602,
                                message: "Missing tool name in tools/call".to_string(),
                                data: None,
                            }),
                            id: req_id,
                        };
                    }
                };

                // Strip the "brain_" tool prefix
                let capability_name = if let Some(stripped) = tool_name.strip_prefix("brain_") {
                    stripped
                } else {
                    return JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(crate::protocol::JsonRpcError {
                            code: -32601, // Method not found
                            message: format!(
                                "Unknown tool name: {}. Must be prefixed with 'brain_'.",
                                tool_name
                            ),
                            data: None,
                        }),
                        id: req_id,
                    };
                };

                let cap = match self.registry.get(capability_name) {
                    Some(c) => c,
                    None => {
                        return JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(crate::protocol::JsonRpcError {
                                code: -32601,
                                message: format!("Tool '{}' is not registered.", tool_name),
                                data: None,
                            }),
                            id: req_id,
                        };
                    }
                };

                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                // Formulate request_token for progress notifications
                let request_token_str = match req_id.as_str() {
                    Some(s) => s.to_string(),
                    None => match req_id.as_i64() {
                        Some(i) => i.to_string(),
                        None => "mcp-call".to_string(),
                    },
                };

                // Configure ExecutionContext
                let context = ExecutionContext::default()
                    .with_cancellation(CancellationToken::new())
                    .with_event_sink(StdioEventSink {
                        request_token: request_token_str,
                        on_notification: self.on_notification.clone(),
                    });

                // Execute capability erased handler
                match cap.execute_erased(&self.app, arguments, &context).await {
                    Ok(res) => {
                        let mcp_res = serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&res).unwrap_or_default()
                                }
                            ]
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: Some(mcp_res),
                            error: None,
                            id: req_id,
                        }
                    }
                    Err(app_err) => {
                        let jsonrpc_err = McpErrorMapper::map(app_err);
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(jsonrpc_err),
                            id: req_id,
                        }
                    }
                }
            }
            _ => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(crate::protocol::JsonRpcError {
                    code: -32601, // Method not found
                    message: format!("Unknown JSON-RPC method: '{}'", req.method),
                    data: None,
                }),
                id: req_id,
            },
        }
    }
}
