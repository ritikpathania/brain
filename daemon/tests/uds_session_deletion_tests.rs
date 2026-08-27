//! Inc 35: UDS integration tests for session deletion & cascade FTS5 purging.
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

struct DaemonProcess {
    child: Child,
    test_dir: PathBuf,
    socket_path: PathBuf,
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.test_dir);
    }
}

fn get_temp_dir() -> PathBuf {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let path = PathBuf::from(format!("/tmp/bd-sess-del-{}", &uuid_str[0..8]));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn get_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn start_test_daemon() -> DaemonProcess {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain_runtime.db");
    let analytics_db_path = test_dir.join("analytics.db");

    let mut cmd = Command::new(bin_path);
    cmd.arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd.spawn().expect("Failed to start daemon process");

    let mut ready = false;
    for _ in 0..60 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "Daemon did not bind socket in time");

    DaemonProcess {
        child,
        test_dir,
        socket_path,
    }
}

async fn rpc_raw(
    socket_path: &PathBuf,
    id: u64,
    action: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": id,
        "action": action,
        "body": serde_json::to_string(&body).unwrap()
    });
    let mut j = serde_json::to_string(&req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    serde_json::from_str(&line).unwrap()
}

async fn rpc(
    socket_path: &PathBuf,
    id: u64,
    action: &str,
    body: serde_json::Value,
) -> (serde_json::Value, String) {
    let frame = rpc_raw(socket_path, id, action, body).await;
    let status = frame["status"].as_str().unwrap_or("unknown").to_string();
    let body_val = if let Some(s) = frame["body"].as_str() {
        serde_json::from_str(s).unwrap_or(frame["body"].clone())
    } else {
        frame["body"].clone()
    };
    (body_val, status)
}

#[tokio::test]
async fn test_session_delete_cascades_persistence_and_search_purging() {
    let daemon = start_test_daemon().await;

    // 1. Create a session
    let (start_res, status_start) = rpc(
        &daemon.socket_path,
        1,
        "v1/session/start",
        serde_json::json!({ "title": "Superconducting Quantum Core" }),
    )
    .await;
    assert_eq!(status_start, "success");
    let sid = start_res["session_id"].as_str().unwrap().to_string();

    // 2. Append turns with distinctive keywords
    let (append_res, status_append) = rpc(
        &daemon.socket_path,
        2,
        "v1/session/append_turn",
        serde_json::json!({
            "session_id": &sid,
            "role": "user",
            "content": "Dilution refrigerators cool transmon qubits down to 15mK."
        }),
    )
    .await;
    println!("APPEND TURN RES: {:?} (status: {})", append_res, status_append);
    assert_eq!(status_append, "success");

    let (comp_res, status_comp) = rpc(
        &daemon.socket_path,
        3,
        "v1/session/complete_turn",
        serde_json::json!({
            "session_id": &sid,
            "assistant_response": "Microwave pulses control state transitions."
        }),
    )
    .await;
    println!("COMP TURN RES: {:?} (status: {})", comp_res, status_comp);
    assert_eq!(status_comp, "success");

    // 3. Verify transcript search indexes the distinctive turns
    let (search_res, status_search) = rpc(
        &daemon.socket_path,
        4,
        "v1/search/query",
        serde_json::json!({ "query": "transmon" }),
    )
    .await;
    println!("SEARCH RES: {:?} (status: {})", search_res, status_search);
    assert_eq!(status_search, "success");
    let matches = search_res["matches"].as_array().expect("search_res should have matches array");
    assert!(!matches.is_empty(), "Expected search to find turn before deletion");

    // 4. Verify session load works before deletion
    let (load_res, status_load) = rpc(
        &daemon.socket_path,
        5,
        "v1/session/load",
        serde_json::json!({ "session_id": &sid }),
    )
    .await;
    println!("LOAD RES: {:?} (status: {})", load_res, status_load);
    assert_eq!(status_load, "success");
    let msgs = load_res.get("messages")
        .or_else(|| load_res.get("session").and_then(|s| s.get("messages")))
        .and_then(|v| v.as_array())
        .expect("load_res should contain messages array");
    assert_eq!(msgs.len(), 2, "Expected 2 messages before deletion");

    // 5. Delete session via v1/session/delete
    let (del_res, status_del) = rpc(
        &daemon.socket_path,
        6,
        "v1/session/delete",
        serde_json::json!({ "session_id": &sid }),
    )
    .await;
    assert_eq!(status_del, "success");
    assert_eq!(del_res["success"], true);
    assert_eq!(del_res["session_id"], sid);

    // 6. Verify session load fails / returns empty messages
    let (load_after, status_load_after) = rpc(
        &daemon.socket_path,
        7,
        "v1/session/load",
        serde_json::json!({ "session_id": &sid }),
    )
    .await;
    let msgs_after = load_after.get("messages")
        .or_else(|| load_after.get("session").and_then(|s| s.get("messages")))
        .and_then(|v| v.as_array());
    let is_empty = status_load_after == "error" || msgs_after.map(|a| a.is_empty()).unwrap_or(true);
    assert!(is_empty, "Session turns should no longer exist after deletion");

    // 7. Verify transcript search returns ZERO matches (purged from FTS5)
    let (search_after, status_search_after) = rpc(
        &daemon.socket_path,
        8,
        "v1/search/query",
        serde_json::json!({ "query": "transmon" }),
    )
    .await;
    assert_eq!(status_search_after, "success");
    let matches_after = search_after["matches"].as_array().unwrap();
    assert_eq!(matches_after.len(), 0, "FTS5 search must return 0 matches for deleted session");

    // Also search title
    let (search_title_res, status_title) = rpc(
        &daemon.socket_path,
        9,
        "v1/search/query",
        serde_json::json!({ "query": "Superconducting" }),
    )
    .await;
    assert_eq!(status_title, "success");
    assert_eq!(search_title_res["matches"].as_array().unwrap().len(), 0, "FTS5 title must be purged");
}

#[tokio::test]
async fn test_session_delete_unversioned_route() {
    let daemon = start_test_daemon().await;

    // Create session
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let req = serde_json::json!({
        "action": "session/start",
        "payload": serde_json::json!({ "title": "Unversioned Delete Test" }).to_string()
    });
    let mut j = serde_json::to_string(&req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    let start_res: serde_json::Value = serde_json::from_str(&line).unwrap();
    println!("UNVERSIONED START RES: {:?}", start_res);
    let sid = start_res.get("session_id")
        .or_else(|| start_res.get("body").and_then(|b| b.get("session_id")))
        .and_then(|v| v.as_str())
        .expect("session_id should exist in unversioned start response")
        .to_string();

    // Delete via unversioned session/delete
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let del_req = serde_json::json!({
        "action": "session/delete",
        "payload": serde_json::json!({ "session_id": sid }).to_string()
    });
    let mut j = serde_json::to_string(&del_req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    let del_res: serde_json::Value = serde_json::from_str(&line).unwrap();
    println!("UNVERSIONED DEL RES: {:?}", del_res);

    assert_eq!(del_res["status"], "ok");
    assert_eq!(del_res["success"], true);
}

#[tokio::test]
async fn test_session_delete_nonexistent() {
    let daemon = start_test_daemon().await;

    let (_, status_del) = rpc(
        &daemon.socket_path,
        99,
        "v1/session/delete",
        serde_json::json!({ "session_id": "01M99999999999999999999999" }),
    )
    .await;
    assert_eq!(status_del, "error");
}
