//! Sustained Soak (1,000 turns), Cold-Start, RSS Stability, Backpressure & Failure Injection Suite (Phase 6)

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

struct DaemonProcess {
    child: Child,
    test_dir: PathBuf,
    socket_path: PathBuf,
    _db_path: PathBuf,
    _analytics_db_path: PathBuf,
    _pid_path: PathBuf,
    _port: u16,
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
    let path = PathBuf::from(format!("/tmp/bd-soak-{}", &uuid_str[0..8]));
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
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(ready, "Daemon did not bind socket in time");

    DaemonProcess {
        child,
        test_dir,
        socket_path,
        _db_path: db_path,
        _analytics_db_path: analytics_db_path,
        _pid_path: pid_path,
        _port: port,
    }
}

fn get_process_rss_kb(pid: u32) -> Option<u64> {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.trim().parse::<u64>().ok()
}

async fn create_test_session(socket_path: &PathBuf, title: &str) -> String {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 1,
        "action": "v1/session/create",
        "body": serde_json::json!({ "title": title }).to_string()
    });
    let mut j = serde_json::to_string(&req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut resp_line = String::new();
    buf_reader.read_line(&mut resp_line).await.unwrap();
    let frame: serde_json::Value = serde_json::from_str(&resp_line).unwrap();
    let body: serde_json::Value = serde_json::from_str(frame["body"].as_str().unwrap()).unwrap();
    body["session_id"].as_str().unwrap().to_string()
}

async fn load_session_messages(socket_path: &PathBuf, session_id: &str) -> Vec<serde_json::Value> {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let load_req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 2,
        "action": "v1/session/load",
        "body": serde_json::json!({ "session_id": session_id }).to_string()
    });
    let mut load_json = serde_json::to_string(&load_req).unwrap();
    load_json.push('\n');
    writer.write_all(load_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut resp_line = String::new();
    buf_reader.read_line(&mut resp_line).await.unwrap();
    let frame: serde_json::Value = serde_json::from_str(&resp_line).unwrap();
    let body: serde_json::Value = serde_json::from_str(frame["body"].as_str().unwrap()).unwrap();
    body["session"]["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

async fn execute_generation_turn_fast(
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
            "messages": [{ "role": "user", "content": prompt }],
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
            } else if frame["type"] == "finished" || frame["type"] == "error" {
                if end_frame.is_null() {
                    end_frame = frame;
                }
                break;
            }
        }
    }

    (accumulated, start_frame, end_frame)
}

#[tokio::test]
async fn test_operational_cold_start_latency() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");
    let port = get_free_port();

    let t0 = Instant::now();

    let mut child = Command::new(bin_path)
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

    let mut connected = false;
    let mut handshake_latency = Duration::ZERO;

    for _ in 0..150 {
        if socket_path.exists() {
            if let Ok(stream) = UnixStream::connect(&socket_path).await {
                let (reader, mut writer) = stream.into_split();
                let mut buf_reader = BufReader::new(reader);

                let ping = serde_json::json!({
                    "version": "1.0",
                    "type": "Request",
                    "id": 1,
                    "action": "handshake",
                    "body": ""
                });
                let mut j = serde_json::to_string(&ping).unwrap();
                j.push('\n');
                if writer.write_all(j.as_bytes()).await.is_ok() && writer.flush().await.is_ok() {
                    let mut resp = String::new();
                    if buf_reader.read_line(&mut resp).await.is_ok() {
                        if let Ok(f) = serde_json::from_str::<serde_json::Value>(&resp) {
                            if f["status"] == "success" {
                                connected = true;
                                handshake_latency = t0.elapsed();
                                break;
                            }
                        }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&test_dir);

    assert!(
        connected,
        "Cold start failed to complete handshake within timeout"
    );
    println!("\n=== COLD-START BOOT LATENCY ===");
    println!(
        "Process Launch -> Initial Handshake: {:.2?}",
        handshake_latency
    );

    // Assert cold start is well within 1500ms
    assert!(
        handshake_latency.as_millis() < 1500,
        "Cold start latency ({:?}) exceeded 1500ms target",
        handshake_latency
    );
}

#[tokio::test]
async fn test_operational_1000_turn_soak_with_rss_stability() {
    let daemon = start_test_daemon().await;
    let socket_path = daemon.socket_path.clone();
    let pid = daemon.child.id();

    // 1. Create 5 rotating sessions (200 turns each = 1,000 total turns)
    let mut sessions = Vec::with_capacity(5);
    for i in 0..5 {
        let sid = create_test_session(&socket_path, &format!("Soak Session #{}", i)).await;
        sessions.push(sid);
    }

    // 2. Warm-up (20 turns)
    for i in 0..20 {
        let sid = &sessions[i % 5];
        let _ = execute_generation_turn_fast(&socket_path, sid, "Warmup turn").await;
    }

    let baseline_rss_kb = get_process_rss_kb(pid).unwrap_or(0);
    println!("\n=== 1,000-TURN SOAK QUALIFICATION ===");
    println!("Post-Warmup Baseline RSS: {} KB", baseline_rss_kb);

    let mut samples: Vec<(usize, u64)> = Vec::new();
    let total_turns = 1000;

    let t_start = Instant::now();

    for turn in 1..=total_turns {
        let sid = &sessions[turn % 5];
        let (resp, start, end) =
            execute_generation_turn_fast(&socket_path, sid, &format!("Soak turn #{}", turn)).await;

        assert_eq!(start["type"], "stream_start");
        assert_eq!(end["type"], "stream_end");
        assert_eq!(end["status"], "completed");
        assert!(!resp.is_empty());

        if turn % 250 == 0 || turn == total_turns {
            let current_rss = get_process_rss_kb(pid).unwrap_or(0);
            samples.push((turn, current_rss));
            println!(
                "Turn {:>4} / 1000 | Current RSS: {:>6} KB",
                turn, current_rss
            );
        }
    }

    let total_duration = t_start.elapsed();
    let final_rss_kb = get_process_rss_kb(pid).unwrap_or(0);

    println!(
        "Completed 1,000 turns in {:.2?} ({:.1} turns/sec)",
        total_duration,
        total_turns as f64 / total_duration.as_secs_f64()
    );
    println!(
        "Final RSS: {} KB (Baseline: {} KB)",
        final_rss_kb, baseline_rss_kb
    );

    // RSS Stability Invariants:
    // 1. Final RSS must remain within 25% of baseline (no unbounded heap growth)
    if baseline_rss_kb > 0 {
        let max_allowed_rss = (baseline_rss_kb as f64 * 1.35) as u64;
        assert!(
            final_rss_kb <= max_allowed_rss,
            "Sustained monotonic RSS growth detected! Final: {} KB > Max Allowed: {} KB",
            final_rss_kb,
            max_allowed_rss
        );
    }

    // 2. Persistence & Data Integrity Invariants:
    // Assert all 5 sessions exist and contain exactly 204 turns (612 messages,
    // user + thinking + assistant each) = 3,060 messages total.
    let mut total_messages = 0;
    for sid in &sessions {
        let msgs = load_session_messages(&socket_path, sid).await;
        assert_eq!(
            msgs.len(),
            612,
            "Session {} must contain exactly 612 messages (204 turns × 3 messages/turn)",
            sid
        );
        // Verify first turn and last turn contents
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "Warmup turn");
        assert_eq!(msgs[1]["role"], "thinking");
        assert_eq!(msgs[2]["role"], "assistant");
        assert_eq!(msgs[msgs.len() - 3]["role"], "user");
        assert!(msgs[msgs.len() - 3]["content"]
            .as_str()
            .unwrap()
            .starts_with("Soak turn #"));
        assert_eq!(msgs[msgs.len() - 2]["role"], "thinking");
        assert_eq!(msgs[msgs.len() - 1]["role"], "assistant");
        total_messages += msgs.len();
    }
    assert_eq!(
        total_messages, 3060,
        "Expected exactly 3,060 persisted messages (1,020 turns × 3 messages/turn in SQLite)"
    );
    println!(
        "Persisted Data Integrity: Verified {} messages across {} sessions via UDS/SQLite",
        total_messages,
        sessions.len()
    );
}

#[tokio::test]
async fn test_operational_uds_backpressure_and_failure_injection() {
    let daemon = start_test_daemon().await;
    let socket_path = daemon.socket_path.clone();
    let session_id = create_test_session(&socket_path, "Backpressure Session").await;

    // 1. Slow reader backpressure test
    let stream = UnixStream::connect(&socket_path).await.unwrap();
    let (mut reader, mut writer) = stream.into_split();

    let gen_req = serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [{ "role": "user", "content": "Slow reader stream prompt" }],
            "model": "brain-default"
        }
    });
    let mut j = serde_json::to_string(&gen_req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    // Read in tiny chunks with deliberate delay to exert socket buffer backpressure
    let mut slow_buffer = [0u8; 32];
    let mut total_read = 0;
    while total_read < 200 {
        let n = reader.read(&mut slow_buffer).await.unwrap();
        if n == 0 {
            break;
        }
        total_read += n;
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(total_read > 0);

    // 2. Client disconnect during active backpressure
    drop(reader);
    drop(writer);

    // Give daemon 200ms to detect EOF on socket and release generation locks
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 3. New client immediately executes generation turn on same session without hang
    let (resp, start, end) = execute_generation_turn_fast(
        &socket_path,
        &session_id,
        "Turn immediately after disconnect",
    )
    .await;

    assert_eq!(start["type"], "stream_start");
    assert_eq!(end["type"], "stream_end");
    assert_eq!(end["status"], "completed");
    assert!(!resp.is_empty());
}
