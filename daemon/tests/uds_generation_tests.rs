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
    let path = PathBuf::from(format!("/tmp/bd-{}", &uuid_str[0..8]));
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

    let child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .env("BRAIN_MOCK_CHUNK_DELAY_MS", "50")
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
    }
}

#[tokio::test]
async fn test_uds_generation_stream_e2e_lifecycle() {
    let daemon = start_test_daemon().await;
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    // 1. Create a session first using versioned RPC request
    let create_sess_req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 1,
        "action": "v1/session/create",
        "body": serde_json::json!({
            "title": "E2E Gen Test Session"
        }).to_string()
    });
    let mut sess_json = serde_json::to_string(&create_sess_req).unwrap();
    sess_json.push('\n');
    writer.write_all(sess_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut sess_resp_line = String::new();
    buf_reader.read_line(&mut sess_resp_line).await.unwrap();
    let sess_resp_frame: serde_json::Value = serde_json::from_str(&sess_resp_line).unwrap();
    let sess_body: serde_json::Value = if let Some(s) = sess_resp_frame["body"].as_str() {
        serde_json::from_str(s).unwrap()
    } else {
        sess_resp_frame["body"].clone()
    };
    let session_id = sess_body["session_id"].as_str().unwrap().to_string();

    // 2. Dispatch v1/generation/stream request
    let gen_id = uuid::Uuid::new_v4().to_string();
    let gen_req = serde_json::json!({
        "id": "req-gen-1",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "generationId": gen_id,
            "messages": [
                { "role": "user", "content": "Hello Brain Engine" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    // 3. Read streamed frames and verify monotonic sequence starting at 0
    let mut frames = Vec::new();
    let mut expected_seq: u64 = 0;

    loop {
        let mut line = String::new();
        if buf_reader.read_line(&mut line).await.unwrap() == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let frame: serde_json::Value = serde_json::from_str(trimmed).unwrap();

        let seq = frame["sequence"].as_u64().unwrap();
        assert_eq!(seq, expected_seq, "Strict sequence monotonicity violated");
        expected_seq += 1;

        let frame_type = frame["type"].as_str().unwrap().to_string();
        frames.push(frame);

        if frame_type == "finished" {
            break;
        }
    }

    assert!(!frames.is_empty());
    assert_eq!(frames[0]["type"], "stream_start");
    assert_eq!(frames[0]["sequence"], 0);

    let last_frame = frames.last().unwrap();
    assert_eq!(last_frame["type"], "finished");
    assert_eq!(last_frame["status"], "completed");

    // 4. Verify session in storage now contains both user and assistant messages
    let load_sess_req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 2,
        "action": "v1/session/load",
        "body": serde_json::json!({
            "session_id": session_id
        }).to_string()
    });
    let mut load_json = serde_json::to_string(&load_sess_req).unwrap();
    load_json.push('\n');
    writer.write_all(load_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut load_resp_line = String::new();
    buf_reader.read_line(&mut load_resp_line).await.unwrap();
    let load_resp_frame: serde_json::Value = serde_json::from_str(&load_resp_line).unwrap();
    let load_body: serde_json::Value = if let Some(s) = load_resp_frame["body"].as_str() {
        serde_json::from_str(s).unwrap()
    } else {
        load_resp_frame["body"].clone()
    };
    let messages = load_body["session"]["messages"].as_array().unwrap();
    // Inc 19: a completed turn also persists its thinking segment.
    assert!(
        messages.len() >= 3,
        "Expected user, thinking, and assistant messages persisted"
    );
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[1]["role"], "thinking");
    assert_eq!(messages[2]["role"], "assistant");
}

#[tokio::test]
async fn test_uds_generation_nonexistent_session_rejected_without_persistence() {
    let daemon = start_test_daemon().await;
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let nonexistent_session_id = brain_domain::SessionId::new().to_string();
    let gen_req = serde_json::json!({
        "id": "req-gen-fake",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": nonexistent_session_id,
            "messages": [
                { "role": "user", "content": "Should fail immediately" }
            ]
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut resp_line = String::new();
    buf_reader.read_line(&mut resp_line).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(&resp_line).unwrap();

    assert_eq!(resp["type"], "error");
    assert_eq!(resp["code"], "session_not_found");
    assert_eq!(resp["status"], "failed");
}

#[tokio::test]
async fn test_uds_generation_cancel_lifecycle() {
    let daemon = start_test_daemon().await;
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    // 1. Create session
    let create_sess_req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 1,
        "action": "v1/session/create",
        "body": serde_json::json!({
            "title": "Cancel Test Session"
        }).to_string()
    });
    let mut sess_json = serde_json::to_string(&create_sess_req).unwrap();
    sess_json.push('\n');
    writer.write_all(sess_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut sess_resp_line = String::new();
    buf_reader.read_line(&mut sess_resp_line).await.unwrap();
    let sess_resp_frame: serde_json::Value = serde_json::from_str(&sess_resp_line).unwrap();
    let sess_body: serde_json::Value = if let Some(s) = sess_resp_frame["body"].as_str() {
        serde_json::from_str(s).unwrap()
    } else {
        sess_resp_frame["body"].clone()
    };
    let session_id = sess_body["session_id"].as_str().unwrap().to_string();

    // 2. Cancel active generation for session
    let cancel_req = serde_json::json!({
        "action": "v1/generation/cancel",
        "payload": serde_json::json!({
            "session_id": session_id
        }).to_string()
    });
    let mut cancel_json = serde_json::to_string(&cancel_req).unwrap();
    cancel_json.push('\n');
    writer.write_all(cancel_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut cancel_resp_line = String::new();
    buf_reader.read_line(&mut cancel_resp_line).await.unwrap();
    let cancel_resp: serde_json::Value = serde_json::from_str(&cancel_resp_line).unwrap();
    assert_eq!(cancel_resp["type"], "cancelled");
    assert_eq!(cancel_resp["status"], "ok");
}
