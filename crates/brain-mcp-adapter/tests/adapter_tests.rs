use brain_application::BrainApplication;
use brain_config::loader::{resolve, DefaultsSource, OverrideSource};
use brain_config::schema::{BrainSettings, PartialBrainSettings, PartialDatabaseSettings};
use brain_mcp_adapter::{JsonRpcRequest, McpAdapter};
use brain_services::BrainRuntime;
use std::sync::Arc;
use std::sync::Mutex;

fn get_temp_db_path() -> String {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    std::env::temp_dir()
        .join(format!("brain_test_mcp_{}.db", uuid_str))
        .to_string_lossy()
        .to_string()
}

fn setup_app() -> Arc<BrainApplication> {
    pyo3::prepare_freethreaded_python();
    let db_path = get_temp_db_path();
    let runtime = BrainRuntime::new(&db_path).unwrap();
    Arc::new(BrainApplication::new(Arc::new(runtime)))
}

#[tokio::test]
async fn test_mcp_initialize_and_tools_list() {
    let app = setup_app();
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let notifications_clone = notifications.clone();

    let adapter = McpAdapter::new(
        app,
        Arc::new(move |notif| {
            notifications_clone.lock().unwrap().push(notif);
        }),
    );

    // 1. Initialize
    let init_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "initialize".to_string(),
        params: None,
        id: Some(serde_json::json!(1)),
    };
    let init_res = adapter.handle_request(init_req).await;
    assert_eq!(init_res.id, serde_json::json!(1));
    assert!(init_res.error.is_none());

    let result = init_res.result.unwrap();
    assert_eq!(
        result.get("protocolVersion").unwrap().as_str().unwrap(),
        "2024-11-05"
    );
    assert_eq!(
        result
            .get("applicationInterface")
            .unwrap()
            .as_str()
            .unwrap(),
        "1.0.0"
    );
    assert!(result
        .get("capabilities")
        .unwrap()
        .get("list")
        .unwrap()
        .is_array());

    // 2. Tools List
    let list_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/list".to_string(),
        params: None,
        id: Some(serde_json::json!(2)),
    };
    let list_res = adapter.handle_request(list_req).await;
    assert_eq!(list_res.id, serde_json::json!(2));

    let list_result = list_res.result.unwrap();
    let tools = list_result.get("tools").unwrap().as_array().unwrap();

    // Assert prefixed names
    let tool_names: Vec<&str> = tools
        .iter()
        .map(|t| t.get("name").unwrap().as_str().unwrap())
        .collect();
    assert!(tool_names.contains(&"brain_search"));
    assert!(tool_names.contains(&"brain_ingest"));
}

#[tokio::test]
async fn test_mcp_tools_call_success_with_progress_notifications() {
    let app = setup_app();
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let notifications_clone = notifications.clone();

    let adapter = McpAdapter::new(
        app,
        Arc::new(move |notif| {
            notifications_clone.lock().unwrap().push(notif);
        }),
    );

    // Valid Ingest call
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(serde_json::json!({
            "name": "brain_ingest",
            "arguments": {
                "event_model_version": "1.0",
                "identity": {
                    "event_id": "a4a7541f-8239-44d4-95e2-b91c0683072c",
                    "parent_event_id": null,
                    "workspace_id": "proj-123",
                    "client_id": "cursor-1.0",
                    "adapter_id": "vscode-ext",
                    "session_id": "01H7X1F8Z9Y000000000000000",
                    "conversation_id": null,
                    "timestamp": "2026-07-12T00:00:00Z"
                },
                "event": {
                    "event_type": "message",
                    "role": "user",
                    "content": "Hello",
                    "metadata": {}
                }
            }
        })),
        id: Some(serde_json::json!("test-token")),
    };

    let res = adapter.handle_request(req).await;
    assert!(res.error.is_none());
    let result = res.result.unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    assert!(text.contains("success"));

    // Verify progress notifications were emitted via event sink
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 3);
    assert_eq!(notifs[0].method, "$/progress");
    assert_eq!(
        notifs[0]
            .params
            .get("progressToken")
            .unwrap()
            .as_str()
            .unwrap(),
        "test-token"
    );
    assert_eq!(notifs[0].params.get("step").unwrap().as_i64().unwrap(), 1);
    assert_eq!(
        notifs[0].params.get("message").unwrap().as_str().unwrap(),
        "Validating ingestion envelope DTO"
    );
}

#[tokio::test]
async fn test_mcp_conformance_error_scenarios() {
    let app = setup_app();
    let adapter = McpAdapter::new(app, Arc::new(move |_| {}));

    // 1. Invalid protocol version
    let bad_version_req = JsonRpcRequest {
        jsonrpc: "1.0".to_string(),
        method: "initialize".to_string(),
        params: None,
        id: Some(serde_json::json!(1)),
    };
    let bad_version_res = adapter.handle_request(bad_version_req).await;
    assert_eq!(bad_version_res.error.unwrap().code, -32600); // Invalid Request

    // 2. Unknown method call
    let unknown_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "invalid/method".to_string(),
        params: None,
        id: Some(serde_json::json!(2)),
    };
    let unknown_res = adapter.handle_request(unknown_req).await;
    assert_eq!(unknown_res.error.unwrap().code, -32601); // Method not found

    // 3. Unknown tool call
    let unknown_tool_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(serde_json::json!({ "name": "brain_nonexistent", "arguments": {} })),
        id: Some(serde_json::json!(3)),
    };
    let unknown_tool_res = adapter.handle_request(unknown_tool_req).await;
    assert_eq!(unknown_tool_res.error.unwrap().code, -32601); // Method/Tool not found

    // 4. Missing tool name
    let missing_name_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(serde_json::json!({ "arguments": {} })),
        id: Some(serde_json::json!(4)),
    };
    let missing_name_res = adapter.handle_request(missing_name_req).await;
    assert_eq!(missing_name_res.error.unwrap().code, -32602); // Invalid Params
}
