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
    let path = PathBuf::from(format!("/tmp/bd-perm-{}", &uuid_str[0..8]));
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

async fn send_frame<T>(writer: &mut tokio::net::unix::OwnedWriteHalf, frame: &T) where T: serde::Serialize {
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

#[tokio::test]
async fn resolve_unknown_call_is_rejected_as_error() {
    let daemon = start_test_daemon().await;
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    send_frame(
        &mut writer,
        &serde_json::json!({
            "id": "req-resolve-bogus",
            "action": "v1/tool/resolve",
            "payload": { "call_id": "no_such_call", "granted": true }
        }),
    )
    .await;

    let reply = read_line_frame(&mut buf_reader).await;
    assert_eq!(reply["status"], "error");
    let body = reply["body"].as_str().unwrap_or_default();
    assert!(
        body.contains("no_such_call"),
        "error should echo the unknown call id, got: {}",
        body
    );
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
            "body": serde_json::json!({ "title": "perm roundtrip" }).to_string()
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

async fn start_generation_with_sentinel(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    session_id: &str,
) {
    send_frame(
        writer,
        &serde_json::json!({
            "id": "req-gen-perm",
            "action": "v1/generation/stream",
            "payload": {
                "sessionId": session_id,
                "generationId": uuid::Uuid::new_v4().to_string(),
                "messages": [
                    { "role": "user", "content": "run [brain-tool:bash|{\"command\":\"ls build\"}] please" }
                ],
                "model": "brain-default"
            }
        }),
    )
    .await;
}

#[tokio::test]
async fn stream_pauses_on_permission_request_and_grant_resumes_without_denial() {
    let daemon = start_test_daemon().await;
    let (mut reader_a, mut writer_a, session_id) =
        open_and_create_session(&daemon.socket_path).await;

    start_generation_with_sentinel(&mut writer_a, &session_id).await;

    // stream_start(0), tool_use(1), tool_permission_requested(2)
    let mut types = Vec::new();
    let mut call_id = String::new();
    for expected_seq in 0u64..3 {
        let frame = read_line_frame(&mut reader_a).await;
        assert_eq!(
            frame["sequence"].as_u64().unwrap(),
            expected_seq,
            "gap in early frames"
        );
        types.push(frame["type"].as_str().unwrap().to_string());
        if frame["type"] == "tool_permission_requested" {
            call_id = frame["call_id"].as_str().unwrap().to_string();
            assert_eq!(frame["tool_name"], "bash");
        }
    }
    assert_eq!(
        types,
        vec!["stream_start", "tool_use", "tool_permission_requested"]
    );

    // Stream must be PAUSED: no further frames within 700 ms.
    let mut probe = String::new();
    let paused = tokio::time::timeout(
        Duration::from_millis(700),
        reader_a.read_line(&mut probe),
    )
    .await;
    assert!(paused.is_err(), "stream continued while permission unresolved");

    // Resolve on a SECOND connection.
    let resolver = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (rreader, mut rwriter) = resolver.into_split();
    let mut rbuf = BufReader::new(rreader);
    send_frame(
        &mut rwriter,
        &serde_json::json!({
            "id": "req-resolve-ok",
            "action": "v1/tool/resolve",
            "payload": { "call_id": call_id, "granted": true }
        }),
    )
    .await;
    let reply = read_line_frame(&mut rbuf).await;
    assert_eq!(reply["status"], "ok");

    // Stream resumes: token frames then finished/completed, NO tool_denied.
    let mut saw_denied = false;
    loop {
        let frame = read_line_frame(&mut reader_a).await;
        match frame["type"].as_str().unwrap() {
            "tool_denied" => saw_denied = true,
            "finished" => {
                assert_eq!(frame["status"], "completed");
                break;
            }
            _ => {}
        }
    }
    assert!(!saw_denied, "grant must not produce tool_denied");
}

#[tokio::test]
async fn denial_emits_tool_denied_then_completes() {
    let daemon = start_test_daemon().await;
    let (mut reader_a, mut writer_a, session_id) =
        open_and_create_session(&daemon.socket_path).await;

    start_generation_with_sentinel(&mut writer_a, &session_id).await;

    let mut call_id = String::new();
    for expected_seq in 0u64..3 {
        let frame = read_line_frame(&mut reader_a).await;
        assert_eq!(frame["sequence"].as_u64().unwrap(), expected_seq);
        if frame["type"] == "tool_permission_requested" {
            call_id = frame["call_id"].as_str().unwrap().to_string();
        }
    }

    let resolver = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (rreader, mut rwriter) = resolver.into_split();
    let mut rbuf = BufReader::new(rreader);
    send_frame(
        &mut rwriter,
        &serde_json::json!({
            "id": "req-resolve-no",
            "action": "v1/tool/resolve",
            "payload": { "call_id": call_id, "granted": false }
        }),
    )
    .await;
    assert_eq!(read_line_frame(&mut rbuf).await["status"], "ok");

    // Expect tool_denied carrying the call id, then finished completed.
    let mut denied_seen = false;
    loop {
        let frame = read_line_frame(&mut reader_a).await;
        match frame["type"].as_str().unwrap() {
            "tool_denied" => {
                denied_seen = true;
                assert_eq!(frame["call_id"].as_str().unwrap(), call_id);
            }
            "finished" => {
                assert_eq!(frame["status"], "completed");
                break;
            }
            _ => {}
        }
    }
    assert!(denied_seen, "deny must emit tool_denied");
}
