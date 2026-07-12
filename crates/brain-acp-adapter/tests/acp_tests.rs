use std::sync::Arc;
use std::sync::Mutex;
use brain_services::ApplicationRuntime;
use brain_application::BrainApplication;
use brain_acp_adapter::{AcpAdapter, AcpRequest};
use brain_config::loader::{resolve, DefaultsSource, OverrideSource};
use brain_config::schema::{BrainSettings, PartialBrainSettings, PartialDatabaseSettings};

fn get_temp_db_path() -> String {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    std::env::temp_dir()
        .join(format!("brain_test_acp_{}.db", uuid_str))
        .to_string_lossy()
        .to_string()
}

fn get_temp_plugins_path() -> String {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let path = std::env::temp_dir().join(format!("brain_test_acp_plugins_{}", uuid_str));
    std::fs::create_dir_all(&path).unwrap();
    path.to_string_lossy().to_string()
}

fn create_valid_test_config(db_path: &str, plugins_path: &str) -> BrainSettings {
    let defaults_src = DefaultsSource;
    let partial = PartialBrainSettings {
        database: Some(PartialDatabaseSettings {
            path: Some(db_path.to_string()),
            pool_size: Some(2),
            enable_wal: Some(false),
        }),
        plugins_directory: Some(plugins_path.to_string()),
        ..Default::default()
    };
    let override_src = OverrideSource::new(partial);
    resolve(&[Box::new(defaults_src), Box::new(override_src)]).unwrap()
}

fn setup_app() -> Arc<BrainApplication> {
    pyo3::prepare_freethreaded_python();
    let db_path = get_temp_db_path();
    let plugins_path = get_temp_plugins_path();
    let config = create_valid_test_config(&db_path, &plugins_path);

    let runtime = ApplicationRuntime::builder()
        .with_config(config)
        .build()
        .unwrap();
    runtime.start().unwrap();
    Arc::new(BrainApplication::new(Arc::new(runtime)))
}

#[tokio::test]
async fn test_acp_initialize_and_session_new() {
    let app = setup_app();
    let adapter = AcpAdapter::new(app, Arc::new(move |_| {}));

    // 1. Initialize
    let init_req = AcpRequest {
        jsonrpc: "2.0".to_string(),
        method: "initialize".to_string(),
        params: Some(serde_json::json!({
            "protocolVersion": "2024-11-05"
        })),
        id: Some(serde_json::json!(1)),
    };
    let init_res = adapter.handle_request(init_req).await.unwrap();
    assert_eq!(init_res.id, serde_json::json!(1));
    assert!(init_res.error.is_none());
    
    let result = init_res.result.unwrap();
    assert_eq!(result.get("protocolVersion").unwrap().as_str().unwrap(), "2024-11-05");
    assert_eq!(result.get("applicationInterface").unwrap().as_str().unwrap(), "1.0.0");
    assert!(result.get("capabilities").unwrap().get("list").unwrap().is_array());

    // 2. Session New
    let new_req = AcpRequest {
        jsonrpc: "2.0".to_string(),
        method: "session/new".to_string(),
        params: Some(serde_json::json!({
            "sessionId": "test-session"
        })),
        id: Some(serde_json::json!(2)),
    };
    let new_res = adapter.handle_request(new_req).await.unwrap();
    assert_eq!(new_res.id, serde_json::json!(2));
    
    let new_result = new_res.result.unwrap();
    assert_eq!(new_result.get("sessionId").unwrap().as_str().unwrap(), "test-session");
}

#[tokio::test]
async fn test_acp_prompt_execution_with_progress_notifications() {
    let app = setup_app();
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let notifications_clone = notifications.clone();
    
    let adapter = AcpAdapter::new(app, Arc::new(move |notif| {
        notifications_clone.lock().unwrap().push(notif);
    }));

    let req = AcpRequest {
        jsonrpc: "2.0".to_string(),
        method: "session/prompt".to_string(),
        params: Some(serde_json::json!({
            "sessionId": "session-123",
            "capability": "ingest",
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
        id: Some(serde_json::json!(1)),
    };

    let res = adapter.handle_request(req).await.unwrap();
    assert!(res.error.is_none());

    let result = res.result.unwrap();
    assert!(result.get("content").unwrap().as_str().unwrap().contains("success"));

    // Verify session/update progress notifications
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 3);
    assert_eq!(notifs[0].method, "session/update");
    assert_eq!(notifs[0].params.get("sessionId").unwrap().as_str().unwrap(), "session-123");
    assert_eq!(notifs[0].params.get("updateType").unwrap().as_str().unwrap(), "progress");
    assert_eq!(notifs[0].params.get("step").unwrap().as_i64().unwrap(), 1);
}

#[tokio::test]
async fn test_acp_conformance_error_scenarios() {
    let app = setup_app();
    let adapter = AcpAdapter::new(app, Arc::new(move |_| {}));

    // 1. Invalid protocol version
    let bad_version_req = AcpRequest {
        jsonrpc: "1.0".to_string(),
        method: "initialize".to_string(),
        params: None,
        id: Some(serde_json::json!(1)),
    };
    let bad_version_res = adapter.handle_request(bad_version_req).await.unwrap();
    assert_eq!(bad_version_res.error.unwrap().code, -32600); // Invalid Request

    // 2. Unknown method call
    let unknown_req = AcpRequest {
        jsonrpc: "2.0".to_string(),
        method: "session/invalid_method".to_string(),
        params: None,
        id: Some(serde_json::json!(2)),
    };
    let unknown_res = adapter.handle_request(unknown_req).await.unwrap();
    assert_eq!(unknown_res.error.unwrap().code, -32601); // Method not found

    // 3. Unknown capability call
    let unknown_tool_req = AcpRequest {
        jsonrpc: "2.0".to_string(),
        method: "session/prompt".to_string(),
        params: Some(serde_json::json!({
            "sessionId": "sess-999",
            "capability": "nonexistent_capability",
            "arguments": {}
        })),
        id: Some(serde_json::json!(3)),
    };
    let unknown_tool_res = adapter.handle_request(unknown_tool_req).await.unwrap();
    assert_eq!(unknown_tool_res.error.unwrap().code, -32601); // Capability not found
}

#[tokio::test]
async fn test_acp_cancellation_flow() {
    let app = setup_app();
    let adapter = AcpAdapter::new(app, Arc::new(move |_| {}));
    
    // 1. Send cancel notification for session-123 when there is no active run
    adapter.handle_notification(
        "session/cancel",
        Some(serde_json::json!({ "sessionId": "session-123" }))
    );
    
    // 2. Test that calling cancel triggers token cancellation and propagates to application
    let adapter_arc = Arc::new(adapter);
    let adapter_clone = adapter_arc.clone();
    
    let handle = tokio::spawn(async move {
        let req = AcpRequest {
            jsonrpc: "2.0".to_string(),
            method: "session/prompt".to_string(),
            params: Some(serde_json::json!({
                "sessionId": "session-cancel-test",
                "capability": "cancel_test",
                "arguments": {}
            })),
            id: Some(serde_json::json!(4)),
        };
        adapter_clone.handle_request(req).await.unwrap()
    });
    
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    
    adapter_arc.handle_notification(
        "session/cancel",
        Some(serde_json::json!({ "sessionId": "session-cancel-test" }))
    );
    
    let res = handle.await.unwrap();
    assert_eq!(res.error.unwrap().code, -32003); // Custom Cancelled Error Code
}

