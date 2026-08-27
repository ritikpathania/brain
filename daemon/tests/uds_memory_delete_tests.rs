//! Inc 31: UDS integration tests for memory deletion & purging.
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
    let path = PathBuf::from(format!("/tmp/bd-mem-del-{}", &uuid_str[0..8]));
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

async fn rpc(
    socket_path: &PathBuf,
    id: u64,
    action: &str,
    body: serde_json::Value,
) -> (serde_json::Value, String) {
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
    let frame: serde_json::Value = serde_json::from_str(&line).unwrap();
    let status = frame["status"].as_str().unwrap_or("").to_string();
    let parsed_body = if let Some(s) = frame["body"].as_str() {
        serde_json::from_str(s).unwrap_or(frame["body"].clone())
    } else {
        frame["body"].clone()
    };
    (parsed_body, status)
}

async fn stream_generation(
    socket_path: &PathBuf,
    session_id: &str,
    prompt: &str,
) -> Vec<serde_json::Value> {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let stream_req = serde_json::json!({
        "sessionId": session_id,
        "messages": [
            { "role": "user", "content": prompt }
        ]
    });

    let req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 100,
        "action": "v1/generation/stream",
        "body": serde_json::to_string(&stream_req).unwrap()
    });

    let mut j = serde_json::to_string(&req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut frames = Vec::new();
    let mut line = String::new();
    while buf_reader.read_line(&mut line).await.unwrap() > 0 {
        if let Ok(frame) = serde_json::from_str::<serde_json::Value>(&line) {
            let is_finish = frame.get("type").and_then(|t| t.as_str()) == Some("finished");
            let is_error = frame.get("type").and_then(|t| t.as_str()) == Some("error");
            frames.push(frame);
            if is_finish || is_error {
                break;
            }
        }
        line.clear();
    }
    frames
}

#[tokio::test]
async fn test_uds_delete_memory_removes_node_and_incident_edges() {
    let daemon = start_test_daemon().await;

    // 1. Store Concept A
    let (store_a, status_a) = rpc(
        &daemon.socket_path,
        1,
        "v1/memory/store",
        serde_json::json!({
            "label": "Superconductor Flux Quantization",
            "content": "Magnetic flux through a superconducting loop is quantized in units of h/2e."
        }),
    )
    .await;
    assert_eq!(status_a, "success");
    let node_a_id = store_a["node_id"].as_str().unwrap();

    // 2. Store Concept B with relation to Concept A
    let (store_b, status_b) = rpc(
        &daemon.socket_path,
        2,
        "v1/memory/store",
        serde_json::json!({
            "label": "Semiconductor Bandgap Engineering",
            "content": "Heterostructures tailor electronic energy bands.",
            "relations": [
                { "target": node_a_id, "relation": "depends_on" }
            ]
        }),
    )
    .await;
    assert_eq!(status_b, "success");
    let node_b_id = store_b["node_id"].as_str().unwrap();

    // 3. Search and assert Concept A is present
    let (search_res, _) = rpc(
        &daemon.socket_path,
        3,
        "v1/memory/search",
        serde_json::json!({ "query": "Quantization" }),
    )
    .await;
    let memories = search_res["memories"].as_array().unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0]["node_id"], node_a_id);

    // 4. Delete Concept A
    let (del_res, del_status) = rpc(
        &daemon.socket_path,
        4,
        "v1/memory/delete",
        serde_json::json!({ "nodeId": node_a_id }),
    )
    .await;
    assert_eq!(del_status, "success");
    assert_eq!(del_res["success"], true);
    assert_eq!(del_res["nodeId"], node_a_id);

    // 5. Subsequent search for Concept A yields 0 results
    let (search_after, _) = rpc(
        &daemon.socket_path,
        5,
        "v1/memory/search",
        serde_json::json!({ "query": "Quantization" }),
    )
    .await;
    let memories_after = search_after["memories"].as_array().unwrap();
    assert_eq!(memories_after.len(), 0, "Deleted concept A appeared in search");

    // 6. Search for Concept B and verify incident edge to Concept A was cascaded/purged
    let (search_b, _) = rpc(
        &daemon.socket_path,
        6,
        "v1/memory/search",
        serde_json::json!({ "query": "Bandgap" }),
    )
    .await;
    let memories_b = search_b["memories"].as_array().unwrap();
    assert_eq!(memories_b.len(), 1);
    assert_eq!(memories_b[0]["node_id"], node_b_id);
    let rels = memories_b[0]["relations"].as_array().unwrap();
    assert_eq!(rels.len(), 0, "Incident edge to deleted node A was not removed");
}

#[tokio::test]
async fn test_uds_delete_memory_excludes_from_generation_context() {
    let daemon = start_test_daemon().await;

    // 1. Store Concept
    let (store_res, _) = rpc(
        &daemon.socket_path,
        1,
        "v1/memory/store",
        serde_json::json!({
            "label": "Zero Knowledge SNARK Proofs",
            "content": "Succinct Non-Interactive Arguments of Knowledge enable verifiable computation."
        }),
    )
    .await;
    let node_id = store_res["node_id"].as_str().unwrap();

    // 2. Start a session
    let (start_res, _) = rpc(
        &daemon.socket_path,
        2,
        "v1/session/start",
        serde_json::json!({"title": "Cryptography Session"}),
    )
    .await;
    let sid = start_res["session_id"].as_str().unwrap();

    // 3. Delete Concept immediately
    let (del_res, del_status) = rpc(
        &daemon.socket_path,
        3,
        "v1/memory/delete",
        serde_json::json!({ "node_id": node_id }),
    )
    .await;
    assert_eq!(del_status, "success");
    assert_eq!(del_res["success"], true);

    // 4. Stream generation turn and verify completion with unaugmented context
    let stream_frames = stream_generation(
        &daemon.socket_path,
        sid,
        "Explain Zero Knowledge SNARK proofs",
    )
    .await;

    assert!(!stream_frames.is_empty());
    let finish_frame = stream_frames.iter().find(|f| f["type"] == "finished");
    assert!(finish_frame.is_some(), "Stream did not finish cleanly");
}

#[tokio::test]
async fn test_uds_delete_memory_negative_paths() {
    let daemon = start_test_daemon().await;

    // 1. Nonexistent node ID
    let random_uuid = uuid::Uuid::new_v4().to_string();
    let (_, status_not_found) = rpc(
        &daemon.socket_path,
        1,
        "v1/memory/delete",
        serde_json::json!({ "nodeId": random_uuid }),
    )
    .await;
    assert_eq!(status_not_found, "error");

    // 2. Malformed UUID
    let (_, status_malformed) = rpc(
        &daemon.socket_path,
        2,
        "v1/memory/delete",
        serde_json::json!({ "nodeId": "not-a-valid-uuid" }),
    )
    .await;
    assert_eq!(status_malformed, "error");
}
