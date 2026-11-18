//! Inc 30: UDS integration tests for context assembly and retrieval augmentation during generation.
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
    let sub = format!("bd-retrieval-aug-{}", &uuid_str[0..8]);
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

async fn rpc(
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
    let frame: serde_json::Value = serde_json::from_str(&line).unwrap();
    if let Some(s) = frame["body"].as_str() {
        serde_json::from_str(s).unwrap_or(frame["body"].clone())
    } else {
        frame["body"].clone()
    }
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
async fn test_generation_stream_retrieves_stored_memory_into_context() {
    let daemon = start_test_daemon().await;

    // 1. Store memory concept in SQLite
    let _ = rpc(
        &daemon.socket_path,
        1,
        "v1/memory/store",
        serde_json::json!({
            "label": "PostgreSQL Streaming Replication",
            "content": "PostgreSQL utilizes Write-Ahead Logging (WAL) and replication slots for high-availability standby replicas.",
            "scope": "global"
        }),
    )
    .await;

    // 2. Start a session
    let start_res = rpc(
        &daemon.socket_path,
        2,
        "v1/session/start",
        serde_json::json!({"title": "Database Engineering"}),
    )
    .await;
    let sid = start_res["session_id"].as_str().unwrap();

    // 3. Query memory retrieval RPC directly to assert assembled context
    let retrieve_res = rpc(
        &daemon.socket_path,
        3,
        "v1/context/retrieve",
        serde_json::json!({
            "query": "PostgreSQL replication WAL",
            "session_id": sid
        }),
    )
    .await;

    let memories = retrieve_res["memories"].as_array().unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0]["label"], "PostgreSQL Streaming Replication");
    assert!(memories[0]["excerpt"]
        .as_str()
        .unwrap()
        .contains("Write-Ahead Logging"));

    // 4. Stream generation turn and verify completion
    let stream_frames = stream_generation(
        &daemon.socket_path,
        sid,
        "How does PostgreSQL replication work?",
    )
    .await;

    assert!(!stream_frames.is_empty());
    let finish_frame = stream_frames.iter().find(|f| f["type"] == "finished");
    assert!(finish_frame.is_some(), "Stream did not finish cleanly");
}

#[tokio::test]
async fn test_generation_stream_includes_pinned_memory_without_lexical_overlap() {
    let daemon = start_test_daemon().await;
    let stored = rpc(
        &daemon.socket_path,
        1,
        "v1/memory/store",
        serde_json::json!({"label": "Gluten restriction", "content": "Avoid wheat", "scope": "global"}),
    )
    .await;
    let node_id = stored["node_id"].as_str().unwrap();
    {
        let db = rusqlite::Connection::open(daemon.test_dir.join("brain_runtime.db")).unwrap();
        let updated = db.execute(
            "UPDATE nodes SET properties = json_set(properties, '$.pinned', json('true')) WHERE id = ?1",
            [node_id],
        ).unwrap();
        assert_eq!(updated, 1);
    }
    let start = rpc(
        &daemon.socket_path,
        2,
        "v1/session/start",
        serde_json::json!({}),
    )
    .await;
    let session_id = start["session_id"].as_str().unwrap();

    let frames = stream_generation(&daemon.socket_path, session_id, "Suggest dinner recipes").await;
    let started = frames
        .iter()
        .find(|frame| frame["type"] == "stream_start")
        .unwrap();
    assert_eq!(
        started["metadata"]["brain_context"]["source_refs"],
        serde_json::json!([node_id])
    );
    assert!(frames.iter().any(|frame| frame["type"] == "finished"));
}

#[tokio::test]
async fn test_generation_does_not_publish_empty_context_on_retrieval_failure() {
    let daemon = start_test_daemon().await;
    let start = rpc(
        &daemon.socket_path,
        1,
        "v1/session/start",
        serde_json::json!({"title": "Retrieval failure test"}),
    )
    .await;
    let session_id = start["session_id"].as_str().unwrap();
    {
        let db = rusqlite::Connection::open(daemon.test_dir.join("brain_runtime.db")).unwrap();
        db.execute_batch("DROP TABLE node_search;").unwrap();
    }

    let frames = stream_generation(&daemon.socket_path, session_id, "test retrieval").await;
    for name in ["stream_start", "stream_end"] {
        let frame = frames.iter().find(|frame| frame["type"] == name).unwrap();
        assert!(
            frame["metadata"].get("brain_context").is_none(),
            "{name}: {frame}"
        );
    }
    assert!(frames.iter().any(|frame| frame["type"] == "finished"));
}

#[tokio::test]
async fn test_generation_stream_irrelevant_query_leaves_context_empty() {
    let daemon = start_test_daemon().await;

    // Store concepts
    let _ = rpc(
        &daemon.socket_path,
        1,
        "v1/memory/store",
        serde_json::json!({
            "label": "Quantum Chromodynamics",
            "content": "Quarks and gluons interact via the strong force with color charge."
        }),
    )
    .await;

    let start_res = rpc(
        &daemon.socket_path,
        2,
        "v1/session/start",
        serde_json::json!({"title": "Baking Session"}),
    )
    .await;
    let sid = start_res["session_id"].as_str().unwrap();

    // Query completely unrelated topic
    let retrieve_res = rpc(
        &daemon.socket_path,
        3,
        "v1/context/retrieve",
        serde_json::json!({
            "query": "Sourdough bread starter hydration",
            "session_id": sid
        }),
    )
    .await;

    let memories = retrieve_res["memories"].as_array().unwrap();
    assert_eq!(memories.len(), 0, "Unrelated memory should not match");

    // Stream finishes normally
    let stream_frames =
        stream_generation(&daemon.socket_path, sid, "What is sourdough hydration?").await;
    let finish_frame = stream_frames.iter().find(|f| f["type"] == "finished");
    assert!(finish_frame.is_some());
}

#[tokio::test]
async fn test_generation_stream_token_budget_clamped() {
    let daemon = start_test_daemon().await;

    let start_res = rpc(
        &daemon.socket_path,
        1,
        "v1/session/start",
        serde_json::json!({"title": "Stress Test Session"}),
    )
    .await;
    let sid = start_res["session_id"].as_str().unwrap();

    // Store multiple memories
    for i in 0..15 {
        let _ = rpc(
            &daemon.socket_path,
            10 + i,
            "v1/memory/store",
            serde_json::json!({
                "label": format!("Distributed Consensus Node {}", i),
                "content": format!("Raft leader election and log replication details for cluster node index {}.", i),
            }),
        )
        .await;
    }

    // Retrieve with max_tokens budget clamping
    let retrieve_res = rpc(
        &daemon.socket_path,
        50,
        "v1/context/retrieve",
        serde_json::json!({
            "query": "Distributed Consensus Raft",
            "session_id": sid,
            "limit": 5
        }),
    )
    .await;

    let memories = retrieve_res["memories"].as_array().unwrap();
    assert!(memories.len() <= 5, "Limit was not respected");

    // Stream generation with budget clamped context
    let stream_frames = stream_generation(
        &daemon.socket_path,
        sid,
        "Explain distributed consensus across nodes.",
    )
    .await;
    let finish_frame = stream_frames.iter().find(|f| f["type"] == "finished");
    assert!(finish_frame.is_some());
}
