//! B5: auto-title backfill end-to-end over UDS, including daemon restart.
//!
//! An untitled session must adopt its first user prompt as its persisted
//! title after one streamed turn; a derived title must survive a hard
//! daemon restart on the same database and never be rewritten by later
//! turns.

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
    pid_path: PathBuf,
    db_path: PathBuf,
    analytics_db_path: PathBuf,
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
    let path = PathBuf::from(format!("/tmp/bd-autotitle-{}", &uuid_str[0..8]));
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

fn spawn_daemon(test_dir: &PathBuf, socket: &PathBuf, pid: &PathBuf, db: &PathBuf, analytics: &PathBuf) -> Child {
    Command::new(env!("CARGO_BIN_EXE_brain-daemon"))
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", socket)
        .env("BRAIN_PID_PATH", pid)
        .env("BRAIN_DB_PATH", db)
        .env("BRAIN_ANALYTICS_DB_PATH", analytics)
        .env("BRAIN_CONFIG_DIR", test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .env("BRAIN_MOCK_CHUNK_DELAY_MS", "10")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon process")
}

async fn start_daemon_at(test_dir: PathBuf) -> DaemonProcess {
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");
    let child = spawn_daemon(&test_dir, &socket_path, &pid_path, &db_path, &analytics_db_path);
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
        pid_path,
        db_path,
        analytics_db_path,
    }
}

/// One versioned RPC round-trip; returns the parsed response body.
/// Mirrors the sibling suites' string-or-object `body` handling.
async fn rpc(socket_path: &PathBuf, id: u64, action: &str, body: serde_json::Value) -> serde_json::Value {
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
        serde_json::from_str(s).unwrap()
    } else {
        frame["body"].clone()
    }
}

/// Streams exactly one generation turn (raw-frame envelope, as in
/// uds_generation_tests.rs) and drains frames until finished/error.
async fn stream_one_turn(socket_path: &PathBuf, session_id: &str, prompt: &str) {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let gen_req = serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "generationId": uuid::Uuid::new_v4().to_string(),
            "messages": [{ "role": "user", "content": prompt }],
            "model": "brain-default"
        }
    });
    let mut j = serde_json::to_string(&gen_req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
    loop {
        let mut line = String::new();
        if buf_reader.read_line(&mut line).await.unwrap() == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(frame) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if frame["type"] == "finished" || frame["type"] == "error" {
                break;
            }
        }
    }
}

#[tokio::test]
async fn test_autotitle_derives_from_first_turn_and_persists() {
    let daemon = start_daemon_at(get_temp_dir()).await;

    // Untitled session via v1/session/create (title omitted -> default).
    let body = rpc(
        &daemon.socket_path,
        1,
        "v1/session/create",
        serde_json::json!({}),
    )
    .await;
    let sid = body["session_id"].as_str().unwrap().to_string();

    stream_one_turn(&daemon.socket_path, &sid, "Help me debug the login flow").await;

    let loaded = rpc(
        &daemon.socket_path,
        2,
        "v1/session/load",
        serde_json::json!({ "session_id": sid }),
    )
    .await;
    assert_eq!(
        loaded["session"]["title"], "Help me debug the login flow",
        "untitled session must adopt its first prompt as title"
    );
}

#[tokio::test]
async fn test_autotitled_title_survives_daemon_restart() {
    let mut daemon = start_daemon_at(get_temp_dir()).await;
    let body = rpc(
        &daemon.socket_path,
        1,
        "v1/session/create",
        serde_json::json!({}),
    )
    .await;
    let sid = body["session_id"].as_str().unwrap().to_string();

    stream_one_turn(&daemon.socket_path, &sid, "Refactor the ingest pipeline").await;

    // Hard-restart on the same DB (pattern from uds_load_stress_tests.rs).
    let _ = daemon.child.kill();
    let _ = daemon.child.wait();
    daemon.child = spawn_daemon(
        &daemon.test_dir,
        &daemon.socket_path,
        &daemon.pid_path,
        &daemon.db_path,
        &daemon.analytics_db_path,
    );
    let mut ready = false;
    for _ in 0..60 {
        if UnixStream::connect(&daemon.socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "Restarted daemon did not bind socket in time");

    let loaded = rpc(
        &daemon.socket_path,
        2,
        "v1/session/load",
        serde_json::json!({ "session_id": sid }),
    )
    .await;
    assert_eq!(
        loaded["session"]["title"], "Refactor the ingest pipeline",
        "derived title must be persisted, not merely in-memory"
    );

    // A titled session is never retitled by later turns.
    stream_one_turn(&daemon.socket_path, &sid, "Second unrelated topic").await;
    let reloaded = rpc(
        &daemon.socket_path,
        3,
        "v1/session/load",
        serde_json::json!({ "session_id": sid }),
    )
    .await;
    assert_eq!(
        reloaded["session"]["title"], "Refactor the ingest pipeline",
        "titled sessions are permanent"
    );
}
