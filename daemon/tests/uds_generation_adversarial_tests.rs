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
    let path = PathBuf::from(format!("/tmp/bd-adv-{}", &uuid_str[0..8]));
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
        db_path,
        analytics_db_path,
        pid_path,
        port,
    }
}

async fn restart_daemon_with_existing_db(process: &mut DaemonProcess) {
    // Kill previous child
    let _ = process.child.kill();
    let _ = process.child.wait();

    // Clean stale socket & pid if any
    let _ = fs::remove_file(&process.socket_path);
    let _ = fs::remove_file(&process.pid_path);

    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let new_port = get_free_port();
    process.port = new_port;

    let child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &process.socket_path)
        .env("BRAIN_PID_PATH", &process.pid_path)
        .env("BRAIN_DB_PATH", &process.db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &process.analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &process.test_dir)
        .env("BRAIN_HEALTH_PORT", new_port.to_string())
        .env("BRAIN_MOCK_CHUNK_DELAY_MS", "50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to restart daemon process");

    process.child = child;

    let mut ready = false;
    for _ in 0..60 {
        if process.socket_path.exists() && UnixStream::connect(&process.socket_path).await.is_ok() {
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
    let sess_body: serde_json::Value = if let Some(s) = sess_resp_frame["body"].as_str() {
        serde_json::from_str(s).unwrap()
    } else {
        sess_resp_frame["body"].clone()
    };
    sess_body["session_id"].as_str().unwrap().to_string()
}

async fn load_session_messages(socket_path: &PathBuf, session_id: &str) -> Vec<serde_json::Value> {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

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
    load_body["session"]["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[tokio::test]
async fn test_scenario_1_client_disconnects_mid_token_cancels_and_does_not_persist_assistant() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Disconnect Test").await;

    // Connect and start generation
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let gen_req = serde_json::json!({
        "id": "req-gen-disc",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [
                { "role": "user", "content": "Disconnect after stream start" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    // Read stream_start frame
    let mut first_line = String::new();
    buf_reader.read_line(&mut first_line).await.unwrap();
    let frame: serde_json::Value = serde_json::from_str(&first_line).unwrap();
    assert_eq!(frame["type"], "stream_start");

    // Abruptly drop client socket
    drop(buf_reader);
    drop(writer);

    // Allow daemon time to detect EOF and cancel
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Verify session in storage: user message persisted, assistant message NOT persisted
    let messages = load_session_messages(&daemon.socket_path, &session_id).await;
    assert_eq!(
        messages.len(),
        1,
        "Expected only user message, but found extra messages"
    );
    assert_eq!(messages[0]["role"], "user");
}

#[tokio::test]
async fn test_scenario_2_simultaneous_cancel_and_completion_produces_exactly_one_terminal_state() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Race Test").await;
    let gen_id = uuid::Uuid::new_v4().to_string();

    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let gen_req = serde_json::json!({
        "id": "req-gen-race",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "generationId": gen_id,
            "messages": [
                { "role": "user", "content": "Race prompt" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    // Race cancel on a second connection immediately
    let socket_path_clone = daemon.socket_path.clone();
    let gen_id_clone = gen_id.clone();
    let sess_id_clone = session_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(5)).await;
        if let Ok(cancel_stream) = UnixStream::connect(&socket_path_clone).await {
            let (_, mut cancel_writer) = cancel_stream.into_split();
            let cancel_req = serde_json::json!({
                "action": "v1/generation/cancel",
                "payload": serde_json::json!({
                    "generation_id": gen_id_clone,
                    "session_id": sess_id_clone
                }).to_string()
            });
            let mut c_json = serde_json::to_string(&cancel_req).unwrap();
            c_json.push('\n');
            let _ = cancel_writer.write_all(c_json.as_bytes()).await;
            let _ = cancel_writer.flush().await;
        }
    });

    // Read all frames until socket closes or finished
    let mut terminal_count = 0;
    let mut terminal_status = String::new();
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
        if let Ok(frame) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(seq) = frame["sequence"].as_u64() {
                assert_eq!(
                    seq, expected_seq,
                    "Sequence monotonicity violated in race test"
                );
                expected_seq += 1;
            }
            if frame["type"] == "finished" {
                terminal_count += 1;
                terminal_status = frame["status"].as_str().unwrap_or("").to_string();
                break;
            }
        }
    }

    // Invariant: Exactly one terminal event reached client
    assert_eq!(terminal_count, 1, "Expected exactly one terminal event");
    assert!(
        terminal_status == "completed" || terminal_status == "cancelled",
        "Terminal status must be either completed or cancelled, got {}",
        terminal_status
    );

    // Verify storage persistence matches the winning terminal state
    let messages = load_session_messages(&daemon.socket_path, &session_id).await;
    if terminal_status == "completed" {
        // Inc 19: a completed turn persists its thinking segment too.
        assert_eq!(
            messages.len(),
            3,
            "Completed generation must persist thinking + assistant messages"
        );
        assert_eq!(messages[1]["role"], "thinking");
        assert_eq!(messages[2]["role"], "assistant");
    } else {
        // Cancel must never persist assistant text. A thinking segment whose
        // ThinkingEnd already landed before the cancel won is allowed (Inc 19).
        assert!(
            !messages.iter().any(|m| m["role"] == "assistant"),
            "Cancelled generation must NOT persist assistant message"
        );
        assert_eq!(messages[0]["role"], "user");
    }
}

#[tokio::test]
async fn test_scenario_9_daemon_crash_and_restart_recovers_cleanly_without_orphan_lock() {
    let mut daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Crash Recovery Test").await;

    // Start generation and read initial chunk
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let gen_req = serde_json::json!({
        "id": "req-gen-crash",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [
                { "role": "user", "content": "Query before crash" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    let frame: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(frame["type"], "stream_start");

    // Abrupt hard crash: SIGKILL daemon
    let pid = daemon.child.id() as i32;
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
    let _ = daemon.child.wait();

    // Restart daemon on SAME database file
    restart_daemon_with_existing_db(&mut daemon).await;

    // Verify recovery state in SQLite:
    let messages = load_session_messages(&daemon.socket_path, &session_id).await;
    assert_eq!(
        messages.len(),
        1,
        "Expected exactly 1 user message after crash restart"
    );
    assert_eq!(messages[0]["role"], "user");

    // Verify session is NOT permanently locked or busy
    let stream2 = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader2, mut writer2) = stream2.into_split();
    let mut buf_reader2 = BufReader::new(reader2);

    let gen_req2 = serde_json::json!({
        "id": "req-gen-post-restart",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [
                { "role": "user", "content": "Query after crash recovery" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json2 = serde_json::to_string(&gen_req2).unwrap();
    gen_json2.push('\n');
    writer2.write_all(gen_json2.as_bytes()).await.unwrap();
    writer2.flush().await.unwrap();

    let mut frames2 = Vec::new();
    loop {
        let mut l = String::new();
        if buf_reader2.read_line(&mut l).await.unwrap() == 0 {
            break;
        }
        let trimmed = l.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(f) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let ftype = f["type"].as_str().unwrap_or("").to_string();
            frames2.push(f);
            if ftype == "finished" {
                break;
            }
        }
    }

    assert!(!frames2.is_empty());
    assert_eq!(frames2.last().unwrap()["type"], "finished");
    assert_eq!(frames2.last().unwrap()["status"], "completed");
}

#[tokio::test]
async fn test_scenario_11_cancel_after_assistant_completion_is_safe_noop() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Post Complete Cancel").await;
    let gen_id = uuid::Uuid::new_v4().to_string();

    // 1. Run generation to completion
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let gen_req = serde_json::json!({
        "id": "req-gen-comp",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "generationId": gen_id,
            "messages": [
                { "role": "user", "content": "Complete first" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    loop {
        let mut line = String::new();
        if buf_reader.read_line(&mut line).await.unwrap() == 0 {
            break;
        }
        if let Ok(f) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            if f["type"] == "finished" {
                break;
            }
        }
    }

    // 2. Cancel AFTER completion
    let cancel_stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (cancel_reader, mut cancel_writer) = cancel_stream.into_split();
    let mut cancel_buf_reader = BufReader::new(cancel_reader);

    let cancel_req = serde_json::json!({
        "action": "v1/generation/cancel",
        "payload": serde_json::json!({
            "generation_id": gen_id,
            "session_id": session_id
        }).to_string()
    });
    let mut c_json = serde_json::to_string(&cancel_req).unwrap();
    c_json.push('\n');
    cancel_writer.write_all(c_json.as_bytes()).await.unwrap();
    cancel_writer.flush().await.unwrap();

    let mut cancel_line = String::new();
    cancel_buf_reader.read_line(&mut cancel_line).await.unwrap();
    let cancel_resp: serde_json::Value = serde_json::from_str(&cancel_line).unwrap();
    assert_eq!(cancel_resp["type"], "cancelled");
    assert_eq!(cancel_resp["status"], "ok");

    // 3. Completed assistant message remains persisted. Inc 19: the turn's
    // thinking segment also persists (it ended before the cancel), so the
    // transcript is user → thinking → assistant.
    let messages = load_session_messages(&daemon.socket_path, &session_id).await;
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[1]["role"], "thinking");
    assert_eq!(messages[2]["role"], "assistant");
}

#[tokio::test]
async fn test_scenario_12_concurrent_generations_on_same_session_rejected_with_session_busy() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Concurrent Same Session").await;

    // Pre-connect both clients to eliminate socket handshake latency
    let stream_a = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader_a, mut writer_a) = stream_a.into_split();
    let mut buf_reader_a = BufReader::new(reader_a);

    let stream_b = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader_b, mut writer_b) = stream_b.into_split();
    let mut buf_reader_b = BufReader::new(reader_b);

    // Start Client A generation
    let gen_req_a = serde_json::json!({
        "id": "req-gen-client-a",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [
                { "role": "user", "content": "Client A prompt" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json_a = serde_json::to_string(&gen_req_a).unwrap();
    gen_json_a.push('\n');
    writer_a.write_all(gen_json_a.as_bytes()).await.unwrap();
    writer_a.flush().await.unwrap();

    // Read first frame from Client A (so Client A is actively registered)
    let mut line_a = String::new();
    buf_reader_a.read_line(&mut line_a).await.unwrap();
    let frame_a: serde_json::Value = serde_json::from_str(&line_a).unwrap();
    assert_eq!(frame_a["type"], "stream_start");

    // Concurrently send Client B generation on SAME session immediately
    let gen_req_b = serde_json::json!({
        "id": "req-gen-client-b",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [
                { "role": "user", "content": "Client B prompt" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json_b = serde_json::to_string(&gen_req_b).unwrap();
    gen_json_b.push('\n');
    writer_b.write_all(gen_json_b.as_bytes()).await.unwrap();
    writer_b.flush().await.unwrap();

    // Client B must be rejected with session_busy
    let mut line_b = String::new();
    buf_reader_b.read_line(&mut line_b).await.unwrap();
    let frame_b: serde_json::Value = serde_json::from_str(&line_b).unwrap();
    assert_eq!(frame_b["type"], "error");
    assert_eq!(frame_b["code"], "session_busy");
    assert_eq!(frame_b["status"], "failed");

    // Client A continues and finishes successfully
    loop {
        let mut l = String::new();
        if buf_reader_a.read_line(&mut l).await.unwrap() == 0 {
            break;
        }
        if let Ok(f) = serde_json::from_str::<serde_json::Value>(l.trim()) {
            if f["type"] == "finished" {
                assert_eq!(f["status"], "completed");
                break;
            }
        }
    }
}

#[tokio::test]
async fn test_scenario_14_malformed_uds_matrix_and_cross_client_isolation() {
    let daemon = start_test_daemon().await;
    let session_a = create_test_session(&daemon.socket_path, "Session A Healthy").await;

    // 1. Client A starts active generation on Session A
    let stream_a = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader_a, mut writer_a) = stream_a.into_split();
    let mut buf_reader_a = BufReader::new(reader_a);

    let gen_req_a = serde_json::json!({
        "id": "req-client-a",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_a,
            "messages": [
                { "role": "user", "content": "Client A prompt during attack" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json_a = serde_json::to_string(&gen_req_a).unwrap();
    gen_json_a.push('\n');
    writer_a.write_all(gen_json_a.as_bytes()).await.unwrap();
    writer_a.flush().await.unwrap();

    let mut line_a = String::new();
    buf_reader_a.read_line(&mut line_a).await.unwrap();
    assert!(line_a.contains("stream_start"));

    // 2. Client B sends a matrix of malformed inputs
    let malformed_inputs: Vec<&[u8]> = vec![
        b"{\"action\": \"unknown/action\", \"payload\": \"abc\"}\n",
        b"{\"id\": 12, \"action\": \"\n", // truncated JSON
        b"{\"unknown\": true, \"random\": [1,2,3]}\n",
        b"{\"action\": \"v1/generation/stream\", \"payload\": { \"sessionId\": \"invalid-ulid\" }}\n",
        b"\n\n\n", // empty lines
    ];

    for attack in malformed_inputs {
        if let Ok(mut attack_stream) = UnixStream::connect(&daemon.socket_path).await {
            let _ = attack_stream.write_all(attack).await;
            let _ = attack_stream.flush().await;
            let mut resp = String::new();
            let mut reader = BufReader::new(attack_stream);
            let _ =
                tokio::time::timeout(Duration::from_millis(200), reader.read_line(&mut resp)).await;
        }
    }

    // 3. Verify Client A's generation was NOT interrupted and completed normally
    let mut completed_a = false;
    loop {
        let mut l = String::new();
        if buf_reader_a.read_line(&mut l).await.unwrap() == 0 {
            break;
        }
        if let Ok(f) = serde_json::from_str::<serde_json::Value>(l.trim()) {
            if f["type"] == "finished" && f["status"] == "completed" {
                completed_a = true;
                break;
            }
        }
    }

    assert!(
        completed_a,
        "Client A's active stream failed to complete due to malformed Client B requests"
    );

    // 4. Verify daemon is still alive and accepting new requests
    let session_c = create_test_session(&daemon.socket_path, "Post Attack Verification").await;
    assert!(!session_c.is_empty());
}
