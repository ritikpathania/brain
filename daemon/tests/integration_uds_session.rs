use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

struct TestDaemon {
    child: Child,
    test_dir: PathBuf,
    socket_path: PathBuf,
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.test_dir);
    }
}

fn get_temp_dir() -> PathBuf {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let sub = format!("bd-sess-{}", &uuid_str[0..8]);
    let tmp_path = std::env::temp_dir().join(&sub);
    if let Ok(()) = fs::create_dir_all(&tmp_path) {
        return tmp_path;
    }
    let alt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/tmp")
        .join(&sub);
    fs::create_dir_all(&alt_path).unwrap();
    alt_path
}

fn get_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn start_daemon_in_dir(test_dir: PathBuf, port: u16) -> (Child, PathBuf, PathBuf) {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain_runtime.db");
    let analytics_db_path = test_dir.join("analytics.db");

    let child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon process");

    let mut ready = false;
    for _ in 0..50 {
        if socket_path.exists() {
            if let Ok(mut stream) = UnixStream::connect(&socket_path).await {
                let req = serde_json::json!({
                    "action": "handshake",
                    "payload": {}
                });
                let mut line = serde_json::to_string(&req).unwrap();
                line.push('\n');
                if stream.write_all(line.as_bytes()).await.is_ok() {
                    let mut reader = BufReader::new(stream);
                    let mut resp = String::new();
                    if reader.read_line(&mut resp).await.is_ok() && !resp.is_empty() {
                        ready = true;
                        break;
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert!(ready, "Daemon did not become ready in time");
    (child, socket_path, db_path)
}

async fn spawn_test_daemon() -> TestDaemon {
    let test_dir = get_temp_dir();
    let port = get_free_port();
    let (child, socket_path, _db_path) = start_daemon_in_dir(test_dir.clone(), port).await;

    TestDaemon {
        child,
        test_dir,
        socket_path,
    }
}

async fn send_uds_rpc(
    socket_path: &PathBuf,
    action: &str,
    payload: serde_json::Value,
) -> serde_json::Value {
    let stream = UnixStream::connect(socket_path)
        .await
        .expect("Failed to connect to UDS socket");
    let (reader, mut writer) = stream.into_split();

    let request = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 1001,
        "action": action,
        "payload": payload,
        "body": serde_json::to_string(&payload).unwrap()
    });

    let mut request_line = serde_json::to_string(&request).unwrap();
    request_line.push('\n');

    writer.write_all(request_line.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut buf_reader = BufReader::new(reader);
    let mut response_line = String::new();
    buf_reader
        .read_line(&mut response_line)
        .await
        .expect("Failed to read RPC response");

    let mut resp: serde_json::Value =
        serde_json::from_str(response_line.trim()).expect("Failed to parse response JSON");
    if let Some(s) = resp.get("body").and_then(|b| b.as_str()) {
        if let Ok(parsed_body) = serde_json::from_str::<serde_json::Value>(s) {
            resp["body"] = parsed_body;
        }
    }
    resp
}

#[tokio::test]
async fn test_uds_session_lifecycle_and_persistence() {
    let mut daemon = spawn_test_daemon().await;
    let socket = daemon.socket_path.clone();

    // 1. session/start
    let start_resp = send_uds_rpc(
        &socket,
        "session/start",
        serde_json::json!({
            "title": "Rust Persistence Test",
            "workspace_path": "/test/workspace"
        }),
    )
    .await;

    assert_eq!(start_resp["status"], "success");
    let session_id = start_resp["body"]["session_id"]
        .as_str()
        .expect("Expected session_id string")
        .to_string();
    assert_eq!(start_resp["body"]["title"], "Rust Persistence Test");

    // 2. session/append_turn (User turn)
    let append_resp = send_uds_rpc(
        &socket,
        "session/append_turn",
        serde_json::json!({
            "session_id": session_id,
            "turn_id": "turn_usr_1",
            "role": "user",
            "content": "Why is Rust memory-safe?"
        }),
    )
    .await;

    assert_eq!(append_resp["status"], "success");
    assert_eq!(append_resp["body"]["success"], true);
    assert_eq!(append_resp["body"]["session_id"], session_id);

    // 3. session/complete_turn (Assistant turn)
    let comp_resp = send_uds_rpc(
        &socket,
        "session/complete_turn",
        serde_json::json!({
            "session_id": session_id,
            "turn_id": "turn_asst_1",
            "assistant_response": "Rust enforces memory safety via ownership and borrow checking at compile time.",
            "duration_ms": 120,
            "input_tokens": 8,
            "output_tokens": 15
        }),
    )
    .await;

    assert_eq!(comp_resp["status"], "success");
    assert_eq!(comp_resp["body"]["success"], true);
    assert_eq!(comp_resp["body"]["total_turns"], 2);

    // 4. memory/store
    let store_resp = send_uds_rpc(
        &socket,
        "memory/store",
        serde_json::json!({
            "session_id": session_id,
            "label": "BorrowCheckerRule",
            "content": "No mutable aliasing allowed",
            "scope": "compiler"
        }),
    )
    .await;

    assert_eq!(store_resp["status"], "success");
    assert_eq!(store_resp["body"]["success"], true);

    // 5. memory/search
    let search_resp = send_uds_rpc(
        &socket,
        "memory/search",
        serde_json::json!({
            "session_id": session_id,
            "query": "BorrowCheckerRule",
            "limit": 5
        }),
    )
    .await;

    assert_eq!(search_resp["status"], "success");
    assert!(
        search_resp["body"]["provenance"]["count"]
            .as_u64()
            .unwrap_or(0)
            >= 1
            || search_resp["body"]["memories"].is_array()
    );

    // 6. KILL DAEMON AND RESTART (Durable Persistence Verification)
    let _ = daemon.child.kill();
    let _ = daemon.child.wait();
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Restart daemon pointing to same test_dir and DB
    let new_port = get_free_port();
    let (new_child, new_socket, _) = start_daemon_in_dir(daemon.test_dir.clone(), new_port).await;
    daemon.child = new_child;
    daemon.socket_path = new_socket.clone();

    // 7. Re-query session messages from storage
    let load_resp = send_uds_rpc(
        &new_socket,
        "v1/session/load",
        serde_json::json!({
            "session_id": session_id
        }),
    )
    .await;

    assert_eq!(load_resp["status"], "success");
    let body_json: serde_json::Value = if load_resp["body"].is_string() {
        serde_json::from_str(load_resp["body"].as_str().unwrap()).unwrap()
    } else {
        load_resp["body"].clone()
    };
    let msgs = body_json["session"]["messages"]
        .as_array()
        .expect("Expected messages array in reloaded session");
    assert_eq!(
        msgs.len(),
        2,
        "Expected 2 messages persisted across daemon restart"
    );
    assert_eq!(msgs[0]["content"], "Why is Rust memory-safe?");
    assert!(msgs[1]["content"].as_str().unwrap().contains("ownership"));
}
