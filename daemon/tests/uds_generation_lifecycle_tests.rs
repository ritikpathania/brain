//! Inc 24: Lifecycle tests for generation guards and cancellation.
//! Pins empty-cancel handling and session unblocking after stream completion.
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
        let dir = self.test_dir.clone();
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(dir);
    }
}

fn get_temp_dir() -> PathBuf {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let path = PathBuf::from(format!("/tmp/bd-lifecycle-{}", &uuid_str[0..8]));
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

async fn start_test_daemon(extra_env: &[(&str, &str)]) -> DaemonProcess {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
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
        .env("BRAIN_MOCK_CHUNK_DELAY_MS", "50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
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

async fn send_frame<T>(writer: &mut tokio::net::unix::OwnedWriteHalf, frame: &T)
where
    T: serde::Serialize,
{
    let mut json = serde_json::to_string(frame).unwrap();
    json.push('\n');
    writer.write_all(json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
}

async fn read_line_frame(reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

/// Opens a connection, creates a session, consumes the create reply, and
/// returns (reader, writer, session_id).
async fn open_and_create_session(
    socket_path: &std::path::Path,
) -> (
    BufReader<tokio::net::unix::OwnedReadHalf>,
    tokio::net::unix::OwnedWriteHalf,
    String,
) {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    send_frame(
        &mut writer,
        &serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 1,
            "action": "v1/session/create",
            "body": serde_json::json!({ "title": "lifecycle test" }).to_string()
        }),
    )
    .await;
    let reply = read_line_frame(&mut buf_reader).await;
    let body: serde_json::Value = if let Some(s) = reply["body"].as_str() {
        serde_json::from_str(s).unwrap()
    } else {
        reply["body"].clone()
    };
    let session_id = body["session_id"].as_str().unwrap().to_string();
    let reader = buf_reader.into_inner();
    (BufReader::new(reader), writer, session_id)
}

async fn start_generation_with_prompt(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    session_id: &str,
    prompt: &str,
) {
    send_frame(
        writer,
        &serde_json::json!({
            "id": "req-gen-lifecycle",
            "action": "v1/generation/stream",
            "payload": {
                "sessionId": session_id,
                "generationId": uuid::Uuid::new_v4().to_string(),
                "messages": [
                    { "role": "user", "content": prompt }
                ],
                "model": "brain-default"
            }
        }),
    )
    .await;
}

#[tokio::test]
async fn cancel_without_active_generations_answers_ok() {
    let proc = start_test_daemon(&[]).await;
    let stream = UnixStream::connect(&proc.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    send_frame(
        &mut writer,
        &serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 7,
            "action": "v1/generation/cancel",
            "body": serde_json::json!({}).to_string()
        }),
    )
    .await;
    let mut buf = BufReader::new(reader);
    let reply = read_line_frame(&mut buf).await;
    assert_eq!(reply["type"], "cancelled", "empty-registry cancel: {reply}");
    assert_eq!(reply["status"], "ok");
}

#[tokio::test]
async fn second_generation_after_completion_is_not_busy_locked() {
    let proc = start_test_daemon(&[]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "first turn").await;
    // Drain to terminal (finished|error); a leaked GenerationGuard here
    // would make the NEXT stream on this session reject session-busy.
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(15), read_line_frame(&mut reader))
            .await
            .expect("terminal frame within 15s");
        let ftype = frame["type"].as_str().unwrap_or("");
        if ftype == "finished" || ftype == "error" {
            break;
        }
    }
    // Same session, second turn: must stream, not answer the busy backstop.
    start_generation_with_prompt(&mut writer, &session_id, "second turn").await;
    let mut saw_terminal = false;
    let mut saw_busy_error = false;
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(15), read_line_frame(&mut reader))
            .await
            .expect("frame within 15s");
        let ftype = frame["type"].as_str().unwrap_or("");
        if ftype == "error" && format!("{frame}").contains("busy") {
            saw_busy_error = true;
            break;
        }
        if ftype == "finished" || ftype == "error" {
            saw_terminal = true;
            break;
        }
    }
    assert!(saw_terminal, "second turn must complete");
    assert!(!saw_busy_error, "guard leak detected: {saw_busy_error}");
}
