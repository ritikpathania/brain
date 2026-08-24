//! Inc 5: post-grant execution. An approved bash call executes daemon-side
//! and exactly one `tool_result` frame rides back on the paused stream;
//! denied calls still produce no result; unknown tools and failing commands
//! come back as error results instead of killing the turn.
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
    let path = PathBuf::from(format!("/tmp/bd-toolexec-{}", &uuid_str[0..8]));
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
            "body": serde_json::json!({ "title": "tool execution" }).to_string()
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
            "id": "req-gen-toolexec",
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

/// Resolves any permission request over a SECOND connection (the stream
/// connection stays parked), asserting the resolver ack. Both halves of the
/// second connection are kept alive until the verdict has been read.
async fn resolve_on_second_connection(
    socket_path: &std::path::Path,
    call_id: &str,
    granted: bool,
) {
    let resolver = UnixStream::connect(socket_path).await.unwrap();
    let (rreader, mut rwriter) = resolver.into_split();
    let mut rbuf = BufReader::new(rreader);
    send_frame(
        &mut rwriter,
        &serde_json::json!({
            "id": "req-resolve",
            "action": "v1/tool/resolve",
            "payload": { "call_id": call_id, "granted": granted }
        }),
    )
    .await;
    let reply = read_line_frame(&mut rbuf).await;
    assert_eq!(reply["status"], "ok", "resolver must ack");
}

/// Drives one generation to completion, resolving any permission request as
/// directed. Returns every frame observed on the stream connection.
async fn run_turn_resolving(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
    socket_path: &std::path::Path,
    granted: bool,
) -> Vec<serde_json::Value> {
    let mut frames = Vec::new();
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(15), read_line_frame(reader))
            .await
            .expect("frame within 15s");
        let ftype = frame["type"].as_str().unwrap_or("").to_string();
        if ftype == "tool_permission_requested" {
            let call_id = frame["call_id"].as_str().unwrap().to_string();
            resolve_on_second_connection(socket_path, &call_id, granted).await;
        }
        // The sibling permission suite treats `finished` as the terminal
        // frame; `stream_end` precedes it inside the stream loop.
        let terminal = ftype == "finished" || ftype == "error";
        frames.push(frame);
        if terminal {
            break;
        }
    }
    frames
}

const GRANT_ECHO_PROMPT: &str =
    "run [brain-tool:bash|{\"command\":\"echo hello-inc5\"}] please";
const DENY_PROMPT: &str =
    "run [brain-tool:bash|{\"command\":\"echo should-not-run\"}] please";
const UNKNOWN_TOOL_PROMPT: &str =
    "run [brain-tool:nosuchtool|{\"command\":\"x\"}] please";
const FAILING_PROMPT: &str =
    "run [brain-tool:bash|{\"command\":\"exit 3\"}] please";

#[tokio::test]
async fn granted_bash_executes_and_emits_tool_result() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, GRANT_ECHO_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &daemon.socket_path, true).await;

    let types: Vec<&str> = frames
        .iter()
        .map(|f| f["type"].as_str().unwrap_or(""))
        .collect();
    let perm_idx = types
        .iter()
        .position(|t| *t == "tool_permission_requested")
        .expect("permission request observed");
    let tr = frames
        .iter()
        .find(|f| f["type"] == "tool_result")
        .expect("granted call must emit tool_result");

    assert_eq!(tr["call_id"], frames[perm_idx]["call_id"]);
    assert_eq!(tr["tool_name"], "bash");
    assert!(
        tr["output"].as_str().unwrap().contains("hello-inc5"),
        "output should carry real command stdout, got: {}",
        tr["output"]
    );
    assert_eq!(tr["is_error"], serde_json::json!(false));
    assert_eq!(tr["exit_code"], serde_json::json!(0));
    // Strictly consecutive: tool_result follows the permission request.
    assert_eq!(
        tr["sequence"].as_i64().unwrap(),
        frames[perm_idx]["sequence"].as_i64().unwrap() + 1
    );
    // Turn still completes normally afterwards.
    assert!(
        types.iter().any(|t| *t == "finished"),
        "turn completes after the result"
    );
}

#[tokio::test]
async fn denied_call_never_emits_tool_result() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, DENY_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &daemon.socket_path, false).await;

    assert!(
        frames.iter().any(|f| f["type"] == "tool_denied"),
        "deny path unchanged"
    );
    assert!(
        !frames.iter().any(|f| f["type"] == "tool_result"),
        "denied call must not execute or emit a result"
    );
}

#[tokio::test]
async fn unknown_tool_yields_error_result_without_execution() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, UNKNOWN_TOOL_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &daemon.socket_path, true).await;

    let tr = frames
        .iter()
        .find(|f| f["type"] == "tool_result")
        .expect("unknown tool still reports a result");
    assert_eq!(tr["is_error"], serde_json::json!(true));
    assert!(tr["output"].as_str().unwrap().contains("Unknown tool"));
}

#[tokio::test]
async fn failing_command_reports_exit_code_as_error_result() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, FAILING_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &daemon.socket_path, true).await;

    let tr = frames
        .iter()
        .find(|f| f["type"] == "tool_result")
        .expect("failing command still reports a result");
    assert_eq!(tr["is_error"], serde_json::json!(true));
    assert_eq!(tr["exit_code"], serde_json::json!(3));
}
