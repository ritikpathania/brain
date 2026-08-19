use brain_a2a_adapter::{A2aAdapter, A2aRequest};
use brain_application::BrainApplication;

use brain_services::BrainRuntime;
use std::sync::Arc;
use std::sync::Mutex;

fn get_temp_db_path() -> String {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    std::env::temp_dir()
        .join(format!("brain_test_a2a_{}.db", uuid_str))
        .to_string_lossy()
        .to_string()
}

fn setup_app() -> Arc<BrainApplication> {
    let db_path = get_temp_db_path();
    let runtime = BrainRuntime::new(&db_path).unwrap();
    Arc::new(BrainApplication::new(Arc::new(runtime)))
}

#[tokio::test]
async fn test_a2a_handshake_and_message() {
    let app = setup_app();
    let adapter = A2aAdapter::new(app, Arc::new(move |_| {}));

    // 1. Handshake
    let handshake_req = A2aRequest {
        jsonrpc: "2.0".to_string(),
        method: "handshake".to_string(),
        params: None,
        id: Some(serde_json::json!(1)),
    };
    let handshake_res = adapter.handle_request(handshake_req).await.unwrap();
    assert_eq!(handshake_res.id, serde_json::json!(1));
    assert!(handshake_res.error.is_none());

    let result = handshake_res.result.unwrap();
    assert_eq!(result.get("version").unwrap().as_str().unwrap(), "1.0.0");
    assert_eq!(
        result
            .get("applicationInterface")
            .unwrap()
            .as_str()
            .unwrap(),
        "1.0.0"
    );
    let capabilities = result.get("capabilities").unwrap().as_array().unwrap();
    assert!(capabilities.iter().any(|c| c.as_str().unwrap() == "search"));

    // 2. Agent/Message (Search)
    let search_req = A2aRequest {
        jsonrpc: "2.0".to_string(),
        method: "agent/message".to_string(),
        params: Some(serde_json::json!({
            "sessionId": "a2a-session-999",
            "capability": "search",
            "arguments": {
                "text": "relational memory database"
            }
        })),
        id: Some(serde_json::json!(2)),
    };
    let search_res = adapter.handle_request(search_req).await.unwrap();
    println!("search_res: {:?}", search_res);
    assert_eq!(search_res.id, serde_json::json!(2));
    assert!(search_res.error.is_none());

    let search_result = search_res.result.unwrap();
    // Verify it is a valid list response
    assert!(search_result.is_array());
}

#[tokio::test]
async fn test_a2a_progress_and_error_conformance() {
    let app = setup_app();
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let notifications_clone = notifications.clone();

    let adapter = A2aAdapter::new(
        app,
        Arc::new(move |notif| {
            notifications_clone.lock().unwrap().push(notif);
        }),
    );

    // 1. Invalid payload DTO on ingest triggers A2A Validation Error (-32602)
    let bad_req = A2aRequest {
        jsonrpc: "2.0".to_string(),
        method: "agent/message".to_string(),
        params: Some(serde_json::json!({
            "sessionId": "a2a-session",
            "capability": "ingest",
            "arguments": {
                "invalid_field": "bad"
            }
        })),
        id: Some(serde_json::json!(3)),
    };
    let bad_res = adapter.handle_request(bad_req).await.unwrap();
    assert_eq!(bad_res.error.unwrap().code, -32602);

    // 2. Unregistered capability call triggers Capability not found (-32601)
    let unregistered_req = A2aRequest {
        jsonrpc: "2.0".to_string(),
        method: "agent/message".to_string(),
        params: Some(serde_json::json!({
            "sessionId": "a2a-session",
            "capability": "nonexistent_capability",
            "arguments": {}
        })),
        id: Some(serde_json::json!(4)),
    };
    let unregistered_res = adapter.handle_request(unregistered_req).await.unwrap();
    assert_eq!(unregistered_res.error.unwrap().code, -32601);
}

#[tokio::test]
async fn test_a2a_cancellation_flow() {
    let app = setup_app();
    let adapter = A2aAdapter::new(app, Arc::new(move |_| {}));

    let adapter_arc = Arc::new(adapter);
    let adapter_clone = adapter_arc.clone();

    let handle = tokio::spawn(async move {
        let req = A2aRequest {
            jsonrpc: "2.0".to_string(),
            method: "agent/message".to_string(),
            params: Some(serde_json::json!({
                "sessionId": "a2a-cancel-test",
                "capability": "cancel_test",
                "arguments": {}
            })),
            id: Some(serde_json::json!(5)),
        };
        adapter_clone.handle_request(req).await.unwrap()
    });

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    adapter_arc.handle_notification(
        "agent/cancel",
        Some(serde_json::json!({ "sessionId": "a2a-cancel-test" })),
    );

    let res = handle.await.unwrap();
    assert_eq!(res.error.unwrap().code, -32003); // Custom Cancelled Error Code
}
