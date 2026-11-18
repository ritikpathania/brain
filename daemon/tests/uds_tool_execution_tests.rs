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
    let sub = format!("bd-toolexec-{}", &uuid_str[0..8]);
    if let Ok(custom) = std::env::var("BRAIN_TEST_TMPDIR") {
        let p = PathBuf::from(custom).join(&sub);
        if fs::create_dir_all(&p).is_ok() {
            return p;
        }
    }
    let sys_tmp = std::env::temp_dir().join(&sub);
    if fs::create_dir_all(&sys_tmp).is_ok() {
        return sys_tmp;
    }
    let tmp_path = PathBuf::from("/tmp").join(&sub);
    if fs::create_dir_all(&tmp_path).is_ok() {
        return tmp_path;
    }
    let local = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/tmp")
        .join(&sub);
    fs::create_dir_all(&local).expect("failed to create any writable temporary directory");
    local
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

    let mut cmd = Command::new(bin_path);
    cmd.arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .env("BRAIN_MOCK_CHUNK_DELAY_MS", "50");

    if let Ok(mode) = std::env::var("BRAIN_SANDBOX_MODE") {
        cmd.env("BRAIN_SANDBOX_MODE", mode);
    } else {
        cmd.env("BRAIN_SANDBOX_MODE", "advisory");
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(dyld) = std::env::var("DYLD_FALLBACK_FRAMEWORK_PATH") {
            cmd.env("DYLD_FALLBACK_FRAMEWORK_PATH", dyld);
        } else {
            cmd.env(
                "DYLD_FALLBACK_FRAMEWORK_PATH",
                "/Library/Developer/CommandLineTools/Library/Frameworks",
            );
        }
    }

    let child = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
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

async fn send_frame<T>(writer: &mut tokio::net::unix::OwnedWriteHalf, frame: &T)
where
    T: serde::Serialize,
{
    let mut json = serde_json::to_string(frame).unwrap();
    json.push('\n');
    writer.write_all(json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
}

async fn read_line_frame(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
) -> serde_json::Value {
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
    permission_request_id: &str,
    session_id: &str,
    generation_id: &str,
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
            "payload": {
                "call_id": call_id,
                "permission_request_id": permission_request_id,
                "session_id": session_id,
                "generation_id": generation_id,
                "granted": granted
            }
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
            let permission_request_id =
                frame["permission_request_id"].as_str().unwrap().to_string();
            let session_id = frame["session_id"].as_str().unwrap().to_string();
            let generation_id = frame["generation_id"].as_str().unwrap().to_string();
            resolve_on_second_connection(
                socket_path,
                &call_id,
                &permission_request_id,
                &session_id,
                &generation_id,
                granted,
            )
            .await;
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

const GRANT_ECHO_PROMPT: &str = "run [brain-tool:bash|{\"command\":\"echo hello-inc5\"}] please";
const DENY_PROMPT: &str = "run [brain-tool:bash|{\"command\":\"echo should-not-run\"}] please";
const UNKNOWN_TOOL_PROMPT: &str = "run [brain-tool:nosuchtool|{\"command\":\"x\"}] please";
const FAILING_PROMPT: &str = "run [brain-tool:bash|{\"command\":\"exit 3\"}] please";

#[tokio::test]
async fn granted_bash_executes_and_emits_tool_result() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, GRANT_ECHO_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &daemon.socket_path, true).await;
    eprintln!("ALL FRAMES in granted_bash: {:#?}", frames);

    let types: Vec<&str> = frames
        .iter()
        .map(|f| f["type"].as_str().unwrap_or(""))
        .collect();
    let perm_idx = types
        .iter()
        .position(|t| *t == "tool_permission_requested")
        .expect("permission request observed");
    assert!(
        frames[perm_idx]["permission_request_id"].as_str().is_some(),
        "daemon must issue an opaque permission request ID"
    );
    let tr = frames
        .iter()
        .find(|f| f["type"] == "tool_result")
        .expect("granted call must emit tool_result");

    assert_eq!(tr["call_id"], frames[perm_idx]["call_id"]);
    assert_eq!(
        tr["permission_request_id"], frames[perm_idx]["permission_request_id"],
        "a terminal tool result must be bound to the permission it settled"
    );
    assert_eq!(tr["tool_name"], "bash");
    assert!(
        tr["output"].as_str().unwrap().contains("hello-inc5"),
        "output should carry real command stdout, got: {}",
        tr["output"]
    );
    assert_eq!(tr["is_error"], serde_json::json!(false));
    assert_eq!(tr["exit_code"], serde_json::json!(0));
    assert_eq!(tr["outcome"], serde_json::json!("success"));
    // Strictly consecutive: tool_result follows the permission request.
    assert_eq!(
        tr["sequence"].as_i64().unwrap(),
        frames[perm_idx]["sequence"].as_i64().unwrap() + 1
    );
    // Turn still completes normally afterwards.
    assert!(
        types.contains(&"finished"),
        "turn completes after the result"
    );
}

#[tokio::test]
async fn denied_call_never_emits_tool_result() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, DENY_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &daemon.socket_path, false).await;

    let denied = frames
        .iter()
        .find(|f| f["type"] == "tool_denied")
        .expect("deny path unchanged");
    let permission = frames
        .iter()
        .find(|f| f["type"] == "tool_permission_requested")
        .expect("permission request");
    assert_eq!(
        denied["permission_request_id"], permission["permission_request_id"],
        "a denial must be bound to the permission it settled"
    );
    assert!(
        !frames.iter().any(|f| f["type"] == "tool_result"),
        "denied call must not execute or emit a result"
    );
}

#[tokio::test]
async fn unknown_tool_yields_error_result_without_execution() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, UNKNOWN_TOOL_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &daemon.socket_path, true).await;
    eprintln!("ALL FRAMES in granted_bash: {:#?}", frames);

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
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, FAILING_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &daemon.socket_path, true).await;
    eprintln!("ALL FRAMES in granted_bash: {:#?}", frames);

    let tr = frames
        .iter()
        .find(|f| f["type"] == "tool_result")
        .expect("failing command still reports a result");
    assert_eq!(tr["is_error"], serde_json::json!(true));
    assert_eq!(tr["exit_code"], serde_json::json!(3));
    assert_eq!(tr["outcome"], serde_json::json!("error"));
}

#[cfg(unix)]
#[tokio::test]
async fn generation_cancellation_reaps_bash_and_emits_no_late_tool_result() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;
    let child_pid_path = daemon.test_dir.join("cancelled-bash.pid");
    let command = format!("echo $$ > {}; exec sleep 60", child_pid_path.display());
    let prompt = format!(
        "run [brain-tool:bash|{}] please",
        serde_json::json!({ "command": command })
    );
    start_generation_with_prompt(&mut writer, &session_id, &prompt).await;

    let mut frames = Vec::new();
    let (call_id, permission_request_id, generation_id) = loop {
        let frame = tokio::time::timeout(Duration::from_secs(10), read_line_frame(&mut reader))
            .await
            .expect("tool permission request within 10s");
        if frame["type"] == "tool_permission_requested" {
            let call_id = frame["call_id"].as_str().unwrap().to_string();
            let permission_request_id =
                frame["permission_request_id"].as_str().unwrap().to_string();
            let generation_id = frame["generation_id"].as_str().unwrap().to_string();
            frames.push(frame);
            break (call_id, permission_request_id, generation_id);
        }
        frames.push(frame);
    };
    resolve_on_second_connection(
        &daemon.socket_path,
        &call_id,
        &permission_request_id,
        &session_id,
        &generation_id,
        true,
    )
    .await;

    let child_pid = wait_for_pid(&child_pid_path).await;
    let cancel_stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (cancel_reader, mut cancel_writer) = cancel_stream.into_split();
    send_frame(
        &mut cancel_writer,
        &serde_json::json!({
            "id": "cancel-running-bash",
            "action": "v1/generation/cancel",
            "payload": serde_json::json!({ "session_id": session_id }).to_string(),
        }),
    )
    .await;
    let mut cancel_reader = BufReader::new(cancel_reader);
    let cancel_reply = read_line_frame(&mut cancel_reader).await;
    assert_eq!(cancel_reply["type"], "cancelled");
    assert_eq!(cancel_reply["status"], "ok");

    loop {
        let frame = tokio::time::timeout(Duration::from_secs(10), read_line_frame(&mut reader))
            .await
            .expect("cancelled stream terminal frame within 10s");
        let terminal = frame["type"] == "finished";
        frames.push(frame);
        if terminal {
            break;
        }
    }

    let terminal = frames.last().unwrap();
    assert_eq!(terminal["type"], "finished");
    assert_eq!(terminal["status"], "cancelled");
    for (expected_sequence, frame) in frames.iter().enumerate() {
        assert_eq!(
            frame["sequence"].as_u64(),
            Some(expected_sequence as u64),
            "cancellation must not burn the unseen tool_result sequence slot: {frames:?}"
        );
    }
    assert!(
        !frames.iter().any(|frame| frame["type"] == "tool_result"),
        "a cancelled bash invocation must not emit a late tool_result: {frames:?}"
    );
    let mut late_frame = String::new();
    match tokio::time::timeout(
        Duration::from_millis(150),
        reader.read_line(&mut late_frame),
    )
    .await
    {
        Ok(Ok(0)) | Err(_) => {}
        Ok(Ok(_)) => panic!(
            "cancelled generation emitted a late frame after its terminal event: {late_frame}"
        ),
        Ok(Err(error)) => panic!("failed while checking for a late generation frame: {error}"),
    }
    assert_process_exited(child_pid).await;
}

#[cfg(unix)]
async fn wait_for_pid(path: &std::path::Path) -> libc::pid_t {
    for _ in 0..100 {
        if let Ok(pid) = fs::read_to_string(path) {
            return pid.trim().parse().unwrap();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("bash process did not publish its PID at {}", path.display());
}

#[cfg(unix)]
async fn assert_process_exited(pid: libc::pid_t) {
    let exited = wait_for_process_exit(pid).await;
    if !exited {
        // Do not leak the child if this regression test fails.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        let _ = wait_for_process_exit(pid).await;
    }
    assert!(
        exited,
        "bash process {pid} survived generation cancellation"
    );
}

#[cfg(unix)]
async fn wait_for_process_exit(pid: libc::pid_t) -> bool {
    for _ in 0..50 {
        let alive = unsafe { libc::kill(pid, 0) == 0 };
        if !alive {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

#[tokio::test]
async fn cross_session_tool_approval_is_rejected_without_executing() {
    let daemon = start_test_daemon().await;
    let (mut reader_a, mut writer_a, session_a) =
        open_and_create_session(&daemon.socket_path).await;
    let (_reader_b, _writer_b, session_b) = open_and_create_session(&daemon.socket_path).await;

    start_generation_with_prompt(&mut writer_a, &session_a, GRANT_ECHO_PROMPT).await;

    let (call_id, perm_id, gen_id) = loop {
        let frame = read_line_frame(&mut reader_a).await;
        if frame["type"] == "tool_permission_requested" {
            break (
                frame["call_id"].as_str().unwrap().to_string(),
                frame["permission_request_id"].as_str().unwrap().to_string(),
                frame["generation_id"].as_str().unwrap().to_string(),
            );
        }
    };

    // Attempt to approve session_a's request using session_b's identity
    let resolver = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (rreader, mut rwriter) = resolver.into_split();
    let mut rbuf = BufReader::new(rreader);
    send_frame(
        &mut rwriter,
        &serde_json::json!({
            "id": "req-resolve-cross-session",
            "action": "v1/tool/resolve",
            "payload": {
                "call_id": call_id,
                "permission_request_id": perm_id,
                "session_id": session_b,
                "generation_id": gen_id,
                "granted": true
            }
        }),
    )
    .await;
    let reply = read_line_frame(&mut rbuf).await;
    assert_eq!(
        reply["status"], "error",
        "cross-session resolution must be rejected"
    );

    // The stream in session A remains waiting; resolve it properly with denial to complete cleanly
    send_frame(
        &mut rwriter,
        &serde_json::json!({
            "id": "req-resolve-valid-denial",
            "action": "v1/tool/resolve",
            "payload": {
                "call_id": call_id,
                "permission_request_id": perm_id,
                "session_id": session_a,
                "generation_id": gen_id,
                "granted": false
            }
        }),
    )
    .await;
    let reply = read_line_frame(&mut rbuf).await;
    assert_eq!(reply["status"], "ok");

    let mut saw_result = false;
    loop {
        let frame = read_line_frame(&mut reader_a).await;
        if frame["type"] == "tool_result" {
            saw_result = true;
        }
        if frame["type"] == "finished" {
            break;
        }
    }
    assert!(
        !saw_result,
        "cross-session attempt must never result in tool execution"
    );
}

#[tokio::test]
async fn replayed_tool_approval_is_rejected() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;

    start_generation_with_prompt(&mut writer, &session_id, GRANT_ECHO_PROMPT).await;

    let (call_id, perm_id, gen_id) = loop {
        let frame = read_line_frame(&mut reader).await;
        if frame["type"] == "tool_permission_requested" {
            break (
                frame["call_id"].as_str().unwrap().to_string(),
                frame["permission_request_id"].as_str().unwrap().to_string(),
                frame["generation_id"].as_str().unwrap().to_string(),
            );
        }
    };

    let resolver = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (rreader, mut rwriter) = resolver.into_split();
    let mut rbuf = BufReader::new(rreader);
    let payload = serde_json::json!({
        "call_id": call_id,
        "permission_request_id": perm_id,
        "session_id": session_id,
        "generation_id": gen_id,
        "tool_name": "bash",
        "granted": true
    });

    // First resolution succeeds
    send_frame(
        &mut rwriter,
        &serde_json::json!({
            "id": "req-resolve-1",
            "action": "v1/tool/resolve",
            "payload": payload
        }),
    )
    .await;
    let reply1 = read_line_frame(&mut rbuf).await;
    assert_eq!(reply1["status"], "ok");

    // Second resolution with identical payload is rejected
    send_frame(
        &mut rwriter,
        &serde_json::json!({
            "id": "req-resolve-2-replay",
            "action": "v1/tool/resolve",
            "payload": payload
        }),
    )
    .await;
    let reply2 = read_line_frame(&mut rbuf).await;
    assert_eq!(
        reply2["status"], "error",
        "replayed resolution must fail closed"
    );
}

#[tokio::test]
async fn mismatched_tool_name_resolution_is_rejected() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;

    start_generation_with_prompt(&mut writer, &session_id, GRANT_ECHO_PROMPT).await;

    let (call_id, perm_id, gen_id) = loop {
        let frame = read_line_frame(&mut reader).await;
        if frame["type"] == "tool_permission_requested" {
            break (
                frame["call_id"].as_str().unwrap().to_string(),
                frame["permission_request_id"].as_str().unwrap().to_string(),
                frame["generation_id"].as_str().unwrap().to_string(),
            );
        }
    };

    let resolver = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (rreader, mut rwriter) = resolver.into_split();
    let mut rbuf = BufReader::new(rreader);

    // Attempt to approve with wrong tool_name
    send_frame(
        &mut rwriter,
        &serde_json::json!({
            "id": "req-resolve-mismatched-tool",
            "action": "v1/tool/resolve",
            "payload": {
                "call_id": call_id,
                "permission_request_id": perm_id,
                "session_id": session_id,
                "generation_id": gen_id,
                "tool_name": "arbitrary_other_tool",
                "granted": true
            }
        }),
    )
    .await;
    let reply = read_line_frame(&mut rbuf).await;
    assert_eq!(
        reply["status"], "error",
        "mismatched tool name must be rejected"
    );
}
