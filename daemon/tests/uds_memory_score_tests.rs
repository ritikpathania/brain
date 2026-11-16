//! Inc 27: UDS integration tests for memory search relevance scores.
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
    let path = PathBuf::from(format!("/tmp/bd-memory-score-{}", &uuid_str[0..8]));
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

#[tokio::test]
async fn test_uds_memory_search_returns_real_relevance_scores() {
    let daemon = start_test_daemon().await;

    // 1. Store a concept in memory
    let store_res = rpc(
        &daemon.socket_path,
        1,
        "v1/memory/store",
        serde_json::json!({
            "label": "Quantum Neural Bridge",
            "content": "Connects core nodes with quantum channels",
            "scope": "workspace"
        }),
    )
    .await;
    assert_eq!(store_res["success"], true);

    // 2. Query v1/memory/search with a specific keyword query
    let search_res = rpc(
        &daemon.socket_path,
        2,
        "v1/memory/search",
        serde_json::json!({
            "query": "Quantum",
            "limit": 10
        }),
    )
    .await;

    let memories = search_res["memories"].as_array().expect("memories array");
    assert!(
        !memories.is_empty(),
        "Expected search results for 'Quantum'"
    );

    let first = &memories[0];
    assert_eq!(first["label"], "Quantum Neural Bridge");
    let score = first["score"].as_i64().expect("score must be an integer");
    // Token overlap adds 50, plus fuzzy score > 0 -> real score > 50
    assert!(
        score >= 50,
        "Expected computed relevance score >= 50, got {}",
        score
    );

    // 3. Query v1/memory/search with empty query -> must retain 100 for blank listing
    let blank_res = rpc(
        &daemon.socket_path,
        3,
        "v1/memory/search",
        serde_json::json!({
            "query": "",
            "limit": 10
        }),
    )
    .await;

    let blank_memories = blank_res["memories"]
        .as_array()
        .expect("blank memories array");
    assert!(
        !blank_memories.is_empty(),
        "Expected stored concept in blank query"
    );
    assert_eq!(blank_memories[0]["label"], "Quantum Neural Bridge");
    assert_eq!(
        blank_memories[0]["score"], 100,
        "Blank query must return score 100"
    );
}
