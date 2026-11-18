//! Inc 29: UDS integration tests for session mutation operations (fork, rename, pin, archive, restore).
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
    let sub = format!("bd-session-mutation-{}", &uuid_str[0..8]);
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
) -> serde_json::Value {
    let frame = rpc_raw(socket_path, id, action, body).await;
    if let Some(s) = frame["body"].as_str() {
        serde_json::from_str(s).unwrap_or(frame["body"].clone())
    } else {
        frame["body"].clone()
    }
}

#[tokio::test]
async fn test_uds_fork_session_clones_history_and_creates_new_session() {
    let daemon = start_test_daemon().await;

    // 1. Start a session
    let start_res = rpc(
        &daemon.socket_path,
        1,
        "v1/session/start",
        serde_json::json!({"title": "Original Parent Session"}),
    )
    .await;
    let source_sid = start_res["session_id"].as_str().unwrap().to_string();

    // 2. Append turns to source session
    let _ = rpc(
        &daemon.socket_path,
        2,
        "v1/session/append_turn",
        serde_json::json!({
            "session_id": &source_sid,
            "role": "user",
            "content": "Hello parent turn"
        }),
    )
    .await;

    let _ = rpc(
        &daemon.socket_path,
        3,
        "v1/session/append_turn",
        serde_json::json!({
            "session_id": &source_sid,
            "role": "assistant",
            "content": "Assistant reply in parent"
        }),
    )
    .await;

    // 3. Fork session via UDS
    let fork_frame = rpc_raw(
        &daemon.socket_path,
        4,
        "v1/session/fork",
        serde_json::json!({
            "source_session_id": &source_sid,
            "new_title": "Forked Child Session"
        }),
    )
    .await;

    assert_eq!(fork_frame["status"], "success");
    let fork_body: serde_json::Value =
        serde_json::from_str(fork_frame["body"].as_str().unwrap()).unwrap();
    let new_sid = fork_body["new_session_id"].as_str().unwrap().to_string();
    assert_ne!(new_sid, source_sid);
    assert_eq!(fork_body["source_session_id"], source_sid);
    assert_eq!(fork_body["cloned_messages_count"], 2);

    // 4. Load the forked session and verify cloned history
    let loaded = rpc(
        &daemon.socket_path,
        5,
        "v1/session/load",
        serde_json::json!({"session_id": &new_sid}),
    )
    .await;

    let session = &loaded["session"];
    let loaded_sid = session["sessionId"]
        .as_str()
        .or_else(|| session["session_id"].as_str())
        .unwrap();
    assert_eq!(loaded_sid, new_sid);
    assert_eq!(session["title"], "Forked Child Session");
    let msgs = session["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["content"], "Hello parent turn");
    assert_eq!(msgs[1]["content"], "Assistant reply in parent");
}

#[tokio::test]
async fn test_uds_rename_session_persists() {
    let daemon = start_test_daemon().await;

    let start_res = rpc(
        &daemon.socket_path,
        1,
        "v1/session/start",
        serde_json::json!({"title": "Old Title"}),
    )
    .await;
    let sid = start_res["session_id"].as_str().unwrap().to_string();

    let rename_frame = rpc_raw(
        &daemon.socket_path,
        2,
        "v1/session/rename",
        serde_json::json!({
            "session_id": &sid,
            "new_title": "Brand New Renamed Title"
        }),
    )
    .await;
    assert_eq!(rename_frame["status"], "success");

    let loaded = rpc(
        &daemon.socket_path,
        3,
        "v1/session/load",
        serde_json::json!({"session_id": &sid}),
    )
    .await;
    assert_eq!(loaded["session"]["title"], "Brand New Renamed Title");

    let list_res = rpc(
        &daemon.socket_path,
        4,
        "v1/session/list",
        serde_json::json!({}),
    )
    .await;
    let sessions = list_res["sessions"].as_array().unwrap();
    let found = sessions
        .iter()
        .find(|s| s["sessionId"] == sid || s["session_id"] == sid)
        .unwrap();
    assert_eq!(found["title"], "Brand New Renamed Title");
}

#[tokio::test]
async fn test_uds_pin_session_updates_flag() {
    let daemon = start_test_daemon().await;

    let start_res = rpc(
        &daemon.socket_path,
        1,
        "v1/session/start",
        serde_json::json!({"title": "Pinnable Session"}),
    )
    .await;
    let sid = start_res["session_id"].as_str().unwrap().to_string();

    // Pin
    let pin_frame = rpc_raw(
        &daemon.socket_path,
        2,
        "v1/session/pin",
        serde_json::json!({
            "session_id": &sid,
            "pinned": true
        }),
    )
    .await;
    assert_eq!(pin_frame["status"], "success");

    let list_res = rpc(
        &daemon.socket_path,
        3,
        "v1/session/list",
        serde_json::json!({}),
    )
    .await;
    let sessions = list_res["sessions"].as_array().unwrap();
    let found = sessions
        .iter()
        .find(|s| s["sessionId"] == sid || s["session_id"] == sid)
        .unwrap();
    assert_eq!(found["pinned"], true);

    // Unpin
    let unpin_frame = rpc_raw(
        &daemon.socket_path,
        4,
        "v1/session/pin",
        serde_json::json!({
            "session_id": &sid,
            "pinned": false
        }),
    )
    .await;
    assert_eq!(unpin_frame["status"], "success");

    let list_res2 = rpc(
        &daemon.socket_path,
        5,
        "v1/session/list",
        serde_json::json!({}),
    )
    .await;
    let sessions2 = list_res2["sessions"].as_array().unwrap();
    let found2 = sessions2
        .iter()
        .find(|s| s["sessionId"] == sid || s["session_id"] == sid)
        .unwrap();
    assert_eq!(found2["pinned"], false);
}

#[tokio::test]
async fn test_uds_archive_and_restore_session() {
    let daemon = start_test_daemon().await;

    let start_res = rpc(
        &daemon.socket_path,
        1,
        "v1/session/start",
        serde_json::json!({"title": "Archivable Session"}),
    )
    .await;
    let sid = start_res["session_id"].as_str().unwrap().to_string();

    // Archive
    let archive_frame = rpc_raw(
        &daemon.socket_path,
        2,
        "v1/session/archive",
        serde_json::json!({"session_id": &sid}),
    )
    .await;
    assert_eq!(archive_frame["status"], "success");

    let list_res = rpc(
        &daemon.socket_path,
        3,
        "v1/session/list",
        serde_json::json!({}),
    )
    .await;
    let sessions = list_res["sessions"].as_array().unwrap();
    let found = sessions
        .iter()
        .find(|s| s["sessionId"] == sid || s["session_id"] == sid)
        .unwrap();
    assert_eq!(found["archived"], true);

    // Restore
    let restore_frame = rpc_raw(
        &daemon.socket_path,
        4,
        "v1/session/restore",
        serde_json::json!({"session_id": &sid}),
    )
    .await;
    assert_eq!(restore_frame["status"], "success");

    let list_res2 = rpc(
        &daemon.socket_path,
        5,
        "v1/session/list",
        serde_json::json!({}),
    )
    .await;
    let sessions2 = list_res2["sessions"].as_array().unwrap();
    let found2 = sessions2
        .iter()
        .find(|s| s["sessionId"] == sid || s["session_id"] == sid)
        .unwrap();
    assert_eq!(found2["archived"], false);
}

#[tokio::test]
async fn test_uds_mutation_negative_paths() {
    let daemon = start_test_daemon().await;
    let unknown_sid = ulid::Ulid::new().to_string();

    // Fork unknown session
    let fork_err = rpc_raw(
        &daemon.socket_path,
        1,
        "v1/session/fork",
        serde_json::json!({
            "source_session_id": &unknown_sid,
            "new_title": "Child"
        }),
    )
    .await;
    assert_eq!(fork_err["status"], "error");
    assert!(fork_err["body"].as_str().unwrap().contains("not found"));

    // Rename unknown session
    let rename_err = rpc_raw(
        &daemon.socket_path,
        2,
        "v1/session/rename",
        serde_json::json!({
            "session_id": &unknown_sid,
            "new_title": "Title"
        }),
    )
    .await;
    assert_eq!(rename_err["status"], "error");
    assert!(rename_err["body"].as_str().unwrap().contains("not found"));

    // Pin unknown session
    let pin_err = rpc_raw(
        &daemon.socket_path,
        3,
        "v1/session/pin",
        serde_json::json!({
            "session_id": &unknown_sid,
            "pinned": true
        }),
    )
    .await;
    assert_eq!(pin_err["status"], "error");
    assert!(pin_err["body"].as_str().unwrap().contains("not found"));

    // Archive unknown session
    let archive_err = rpc_raw(
        &daemon.socket_path,
        4,
        "v1/session/archive",
        serde_json::json!({"session_id": &unknown_sid}),
    )
    .await;
    assert_eq!(archive_err["status"], "error");
    assert!(archive_err["body"].as_str().unwrap().contains("not found"));

    // Restore unknown session
    let restore_err = rpc_raw(
        &daemon.socket_path,
        5,
        "v1/session/restore",
        serde_json::json!({"session_id": &unknown_sid}),
    )
    .await;
    assert_eq!(restore_err["status"], "error");
    assert!(restore_err["body"].as_str().unwrap().contains("not found"));
}
