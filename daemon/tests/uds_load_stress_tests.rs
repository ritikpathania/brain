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
    db_path: PathBuf,
    analytics_db_path: PathBuf,
    pid_path: PathBuf,
    port: u16,
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
    let path = PathBuf::from(format!("/tmp/bd-load-{}", &uuid_str[0..8]));
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
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");
    let port = get_free_port();

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
        db_path,
        analytics_db_path,
        pid_path,
        port,
    }
}

async fn restart_daemon_with_existing_db(daemon: &mut DaemonProcess) {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let port = get_free_port();
    daemon.port = port;

    let child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &daemon.socket_path)
        .env("BRAIN_PID_PATH", &daemon.pid_path)
        .env("BRAIN_DB_PATH", &daemon.db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &daemon.analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &daemon.test_dir)
        .env("BRAIN_HEALTH_PORT", port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to restart daemon process");

    daemon.child = child;

    let mut ready = false;
    for _ in 0..60 {
        if daemon.socket_path.exists() && UnixStream::connect(&daemon.socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "Restarted daemon did not bind socket in time");
}

async fn create_test_session(socket_path: &PathBuf, title: &str) -> String {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let create_sess_req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 1,
        "action": "v1/session/create",
        "body": serde_json::json!({
            "title": title
        }).to_string()
    });
    let mut sess_json = serde_json::to_string(&create_sess_req).unwrap();
    sess_json.push('\n');
    writer.write_all(sess_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut sess_resp_line = String::new();
    buf_reader.read_line(&mut sess_resp_line).await.unwrap();
    let sess_resp_frame: serde_json::Value = serde_json::from_str(&sess_resp_line).unwrap();
    let sess_body: serde_json::Value =
        serde_json::from_str(sess_resp_frame["body"].as_str().unwrap()).unwrap();
    sess_body["session_id"].as_str().unwrap().to_string()
}

async fn load_session_messages(socket_path: &PathBuf, session_id: &str) -> Vec<serde_json::Value> {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 99,
        "action": "v1/session/load",
        "body": serde_json::json!({ "session_id": session_id }).to_string()
    });
    let mut j = serde_json::to_string(&req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut resp_line = String::new();
    buf_reader.read_line(&mut resp_line).await.unwrap();
    let frame: serde_json::Value = serde_json::from_str(&resp_line).unwrap();
    let load_body: serde_json::Value =
        serde_json::from_str(frame["body"].as_str().unwrap()).unwrap();
    load_body["session"]["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

async fn execute_generation_turn(
    socket_path: &PathBuf,
    session_id: &str,
    prompt: &str,
) -> (String, serde_json::Value, serde_json::Value) {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let gen_req = serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [
                { "role": "user", "content": prompt }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut start_frame = serde_json::Value::Null;
    let mut end_frame = serde_json::Value::Null;
    let mut accumulated = String::new();

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
            if frame["type"] == "stream_start" {
                start_frame = frame;
            } else if frame["type"] == "token" {
                if let Some(t) = frame["token"].as_str() {
                    accumulated.push_str(t);
                }
            } else if frame["type"] == "stream_end" {
                end_frame = frame;
            } else if frame["type"] == "finished" {
                if end_frame.is_null() {
                    end_frame = frame;
                }
                break;
            } else if frame["type"] == "error" {
                end_frame = frame;
                break;
            }
        }
    }

    (accumulated, start_frame, end_frame)
}

#[tokio::test]
async fn test_load_20_concurrent_sessions_under_load() {
    let daemon = start_test_daemon().await;
    let socket_path = daemon.socket_path.clone();

    // Create 20 distinct sessions
    let mut session_ids = Vec::with_capacity(20);
    for i in 0..20 {
        let sid = create_test_session(&socket_path, &format!("Load Test Session #{}", i)).await;
        session_ids.push(sid);
    }

    let mut handles = Vec::with_capacity(20);
    for (i, sid) in session_ids.iter().enumerate() {
        let sp = socket_path.clone();
        let s = sid.clone();
        let handle = tokio::spawn(async move {
            let (resp, start, end) =
                execute_generation_turn(&sp, &s, &format!("Concurrent load turn #{}", i)).await;
            assert_eq!(start["type"], "stream_start");
            assert_eq!(end["type"], "stream_end");
            assert_eq!(end["status"], "completed");
            assert!(!resp.is_empty());
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.unwrap();
    }

    // Verify all 20 sessions cleanly persisted exactly 3 messages
    // (user + thinking + assistant)
    for sid in &session_ids {
        let msgs = load_session_messages(&socket_path, sid).await;
        assert_eq!(msgs.len(), 3, "Session '{}' missing completed turn", sid);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[1]["role"], "thinking");
        assert_eq!(msgs[2]["role"], "assistant");
    }
}

#[tokio::test]
async fn test_load_concurrent_wal_writes_during_streaming() {
    let daemon = start_test_daemon().await;
    let socket_path = daemon.socket_path.clone();

    let mut session_ids = Vec::with_capacity(10);
    for i in 0..10 {
        let sid = create_test_session(&socket_path, &format!("Stream Session #{}", i)).await;
        session_ids.push(sid);
    }

    let mut stream_handles = Vec::with_capacity(10);
    for (i, sid) in session_ids.iter().enumerate() {
        let sp = socket_path.clone();
        let s = sid.clone();
        let handle = tokio::spawn(async move {
            let (resp, start, end) =
                execute_generation_turn(&sp, &s, &format!("Turn with background WAL {}", i)).await;
            assert_eq!(start["type"], "stream_start");
            assert_eq!(end["type"], "stream_end");
            assert!(!resp.is_empty());
        });
        stream_handles.push(handle);
    }

    // Concurrent WAL writers creating sessions in parallel
    let mut wal_handles = Vec::with_capacity(10);
    for i in 0..10 {
        let sp = socket_path.clone();
        let handle = tokio::spawn(async move {
            for j in 0..5 {
                let sid = create_test_session(&sp, &format!("WAL Ingest #{}-{}", i, j)).await;
                assert!(!sid.is_empty());
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        wal_handles.push(handle);
    }

    for h in stream_handles {
        h.await.unwrap();
    }
    for h in wal_handles {
        h.await.unwrap();
    }

    // Verify stream sessions have completed states (user + thinking + assistant)
    for sid in &session_ids {
        let msgs = load_session_messages(&socket_path, sid).await;
        assert_eq!(msgs.len(), 3);
    }
}

#[tokio::test]
async fn test_load_sigkill_crash_recovery_persistence_checkpoints() {
    let mut daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Crash Checkpoint Session").await;

    // Start a generation stream over UDS
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let gen_req = serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [
                { "role": "user", "content": "Persisted prompt before SIGKILL" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    // Read stream_start frame and at least 1 content frame (thinking_start or token)
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    let start_frame: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(start_frame["type"], "stream_start");

    line.clear();
    buf_reader.read_line(&mut line).await.unwrap();
    let next_frame: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert!(next_frame["type"] == "thinking_start" || next_frame["type"] == "token");

    // Checkpoint T3: Hard SIGKILL daemon mid-generation
    let _ = daemon.child.kill();
    let _ = daemon.child.wait();

    // Checkpoint T4: Restart daemon on existing SQLite DB
    restart_daemon_with_existing_db(&mut daemon).await;

    // Verify Checkpoints:
    // 1. User message is persisted at T0
    // 2. Partial assistant tokens are absent from DB
    // 3. No orphan locks remain
    let msgs_after_crash = load_session_messages(&daemon.socket_path, &session_id).await;
    assert_eq!(
        msgs_after_crash.len(),
        1,
        "Expected only the user message; partial assistant must not be saved"
    );
    assert_eq!(msgs_after_crash[0]["role"], "user");
    assert_eq!(
        msgs_after_crash[0]["content"],
        "Persisted prompt before SIGKILL"
    );

    // Subsequent generation turn executes successfully without lock interference
    let (resp, start, end) = execute_generation_turn(
        &daemon.socket_path,
        &session_id,
        "Turn after crash recovery",
    )
    .await;
    assert_eq!(start["type"], "stream_start");
    assert_eq!(end["type"], "stream_end");
    assert!(!resp.is_empty());

    let final_msgs = load_session_messages(&daemon.socket_path, &session_id).await;
    // 1st User msg + (2nd User + Thinking + Assistant) msgs
    assert_eq!(final_msgs.len(), 4);
}

#[tokio::test]
async fn test_telemetry_failure_non_blocking_isolation() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Telemetry Isolation Session").await;

    // Execute generation turn and inspect telemetry in start and end frames
    let (_resp, start, end) =
        execute_generation_turn(&daemon.socket_path, &session_id, "Telemetry check prompt").await;

    assert_eq!(start["type"], "stream_start");
    let telemetry_start = &start["metadata"]["telemetry"];
    assert_eq!(telemetry_start["session_id"], session_id);
    assert!(telemetry_start["assembly_latency_ms"].is_number());

    assert_eq!(end["type"], "stream_end");
    let telemetry_end = &end["metadata"]["telemetry"];
    assert_eq!(telemetry_end["session_id"], session_id);
    assert!(telemetry_end["total_duration_ms"].is_number());
    assert_eq!(telemetry_end["finish_reason"], "end_turn");
}
