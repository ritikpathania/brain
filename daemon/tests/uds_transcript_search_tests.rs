//! Inc 32: UDS integration tests for cross-session transcript search.
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
    let path = PathBuf::from(format!("/tmp/bd-search-{}", &uuid_str[0..8]));
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
async fn test_uds_transcript_search_matches_historical_messages_across_sessions() {
    let daemon = start_test_daemon().await;

    // 1. Create Session A with a unique technical message
    let (start_a, status_a) = rpc(
        &daemon.socket_path,
        1,
        "v1/session/start",
        serde_json::json!({ "title": "Superconducting Research Topic" }),
    )
    .await;
    assert_eq!(status_a, "success");
    let session_a_id = start_a["session_id"].as_str().unwrap().to_string();

    let (_, status_append_a) = rpc(
        &daemon.socket_path,
        2,
        "v1/session/append_turn",
        serde_json::json!({
            "session_id": &session_a_id,
            "role": "user",
            "content": "Superconducting circuits utilize Josephson junctions for non-linear inductance."
        }),
    )
    .await;
    assert_eq!(status_append_a, "success");

    let (_, status_comp_a) = rpc(
        &daemon.socket_path,
        3,
        "v1/session/complete_turn",
        serde_json::json!({
            "session_id": &session_a_id,
            "assistant_response": "Josephson junctions enable dissipationless non-linear inductive reactance."
        }),
    )
    .await;
    assert_eq!(status_comp_a, "success");

    // 2. Create Session B with a different technical message
    let (start_b, status_b) = rpc(
        &daemon.socket_path,
        4,
        "v1/session/start",
        serde_json::json!({ "title": "Trapped Ion Research Topic" }),
    )
    .await;
    assert_eq!(status_b, "success");
    let session_b_id = start_b["session_id"].as_str().unwrap().to_string();

    let (_, status_append_b) = rpc(
        &daemon.socket_path,
        5,
        "v1/session/append_turn",
        serde_json::json!({
            "session_id": &session_b_id,
            "role": "user",
            "content": "Trapped ion hardware uses optical lasers to manipulate ytterbium hyperfine levels."
        }),
    )
    .await;
    assert_eq!(status_append_b, "success");

    let (_, status_comp_b) = rpc(
        &daemon.socket_path,
        6,
        "v1/session/complete_turn",
        serde_json::json!({
            "session_id": &session_b_id,
            "assistant_response": "Ytterbium ions provide high fidelity single and two-qubit gate operations."
        }),
    )
    .await;
    assert_eq!(status_comp_b, "success");

    // 3. Search for "Josephson" -> match should map to Session A
    let (res_a, status_search_a) = rpc(
        &daemon.socket_path,
        7,
        "v1/search/query",
        serde_json::json!({ "query": "Josephson" }),
    )
    .await;
    assert_eq!(status_search_a, "success");
    let matches_a = res_a["matches"].as_array().expect("matches array");
    assert!(
        !matches_a.is_empty(),
        "Must find at least one match for Josephson"
    );
    // Verify match belongs to Session A
    let match_msg = &matches_a[0];
    let sid = match_msg["sessionId"]
        .as_str()
        .or_else(|| match_msg["session_id"].as_str());
    assert_eq!(sid, Some(session_a_id.as_str()));

    // 4. Search for "ytterbium" -> match should map to Session B
    let (res_b, status_search_b) = rpc(
        &daemon.socket_path,
        8,
        "v1/search/query",
        serde_json::json!({ "query": "ytterbium" }),
    )
    .await;
    assert_eq!(status_search_b, "success");
    let matches_b = res_b["matches"].as_array().expect("matches array");
    assert!(
        !matches_b.is_empty(),
        "Must find at least one match for ytterbium"
    );
    let match_b_msg = &matches_b[0];
    let sid_b = match_b_msg["sessionId"]
        .as_str()
        .or_else(|| match_b_msg["session_id"].as_str());
    assert_eq!(sid_b, Some(session_b_id.as_str()));
}

#[tokio::test]
async fn test_uds_transcript_search_unmatched_query_returns_empty() {
    let daemon = start_test_daemon().await;

    let (res, status) = rpc(
        &daemon.socket_path,
        1,
        "v1/search/query",
        serde_json::json!({ "query": "nonexistent_cryptographic_quantum_entropy" }),
    )
    .await;
    assert_eq!(status, "success");
    let matches = res["matches"].as_array().expect("matches array");
    assert_eq!(matches.len(), 0);
    assert_eq!(res["total"], 0);
}

#[tokio::test]
async fn test_uds_transcript_search_negative_and_boundary_cases() {
    let daemon = start_test_daemon().await;

    // 1. Empty query string returns error
    let (res_err, status_err) = rpc(
        &daemon.socket_path,
        1,
        "v1/search/query",
        serde_json::json!({ "query": "   " }),
    )
    .await;
    assert_eq!(status_err, "error");
    assert!(
        res_err.as_str().unwrap().contains("must not be empty"),
        "Expected validation error message"
    );

    // 2. Pagination limit clamping works
    let (res_lim, status_lim) = rpc(
        &daemon.socket_path,
        2,
        "v1/search/query",
        serde_json::json!({ "query": "test", "limit": 1 }),
    )
    .await;
    assert_eq!(status_lim, "success");
    let matches = res_lim["matches"].as_array().expect("matches array");
    assert!(matches.len() <= 1);
}
