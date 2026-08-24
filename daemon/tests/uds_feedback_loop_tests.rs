//! Increment 6: the agentic feedback loop — tool results feed back into the
//! same turn.
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
    let path = PathBuf::from(format!("/tmp/bd-feedback-{}", &uuid_str[0..8]));
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
            "body": serde_json::json!({ "title": "feedback loop" }).to_string()
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
            "id": "req-gen-feedback",
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

/// Round 1 emits text + one bash call (finish "tool_use"); round 2 sees the
/// fed-back result and finishes cleanly. Byte lengths: "Round one text." ==
/// 15, "Round two wraps up." == 19; mock hardcodes input_tokens 15 per pass.
const TWO_ROUND_SCRIPT: &str = r#"[{"tokens":["Round one text."],"tool_calls":[["call_fb_1","bash",{"command":"echo feedback-round-one"}]],"finish_reason":"tool_use"},{"tokens":["Round two wraps up."],"finish_reason":"end_turn"}]"#;

#[tokio::test]
async fn two_round_turn_feeds_result_back_and_finishes_cleanly() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", TWO_ROUND_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "run the scripted loop").await;

    let frames = run_turn_resolving(&mut reader, &proc.socket_path, true).await;

    let types: Vec<&str> = frames
        .iter()
        .filter_map(|f| f["type"].as_str())
        .collect();
    // Strictly consecutive sequences across BOTH passes — every frame,
    // including the terminal stream_end, owns exactly one slot.
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(
            f["sequence"].as_u64().unwrap_or_else(|| panic!("frame {i} missing sequence")),
            i as u64,
            "frame {i} ({}) broke consecutiveness",
            types[i]
        );
    }

    // Exactly one executed result; nothing denied.
    assert_eq!(types.iter().filter(|t| **t == "tool_result").count(), 1);
    assert!(!types.contains(&"tool_denied"));

    // Round-2 text arrives AFTER the tool_result frame (fed-back continuation).
    let result_idx = types.iter().position(|t| *t == "tool_result").unwrap();
    let round_two_idx = frames
        .iter()
        .position(|f| {
            f["type"] == "token"
                && f["token"].as_str().unwrap_or("").contains("Round two wraps up.")
        })
        .unwrap();
    assert!(round_two_idx > result_idx);

    // Inc 10: the live wire frame carries the same daemon-measured duration
    // that Inc 8 persists in the tool_event envelope — one clock, both views.
    let result_frame = &frames[result_idx];
    assert!(
        result_frame["duration_ms"].is_u64(),
        "tool_result frame missing duration_ms: {result_frame}"
    );

    // stream_end carries both passes' text, summed usage, clean finish.
    let end = frames
        .iter()
        .find(|f| f["type"] == "stream_end")
        .expect("stream_end present");
    let response = end["response"].as_str().unwrap();
    assert!(response.contains("Round one text."), "response: {response}");
    assert!(response.contains("Round two wraps up."), "response: {response}");
    assert_eq!(end["finish_reason"], "end_turn");
    assert_eq!(end["metadata"]["inputTokens"], 30);
    assert_eq!(end["metadata"]["outputTokens"], 34);
    assert_eq!(types.last(), Some(&"finished"));
}

/// Collects frames until the terminal event WITHOUT resolving permissions
/// (turns that request nothing never park).
async fn run_turn(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
) -> Vec<serde_json::Value> {
    let mut frames = Vec::new();
    loop {
        let f = tokio::time::timeout(Duration::from_secs(10), read_line_frame(reader))
            .await
            .expect("frame timeout");
        let ftype = f["type"].as_str().unwrap_or("").to_string();
        frames.push(f);
        if ftype == "finished" || ftype == "error" {
            return frames;
        }
    }
}

const DENY_SCRIPT: &str = TWO_ROUND_SCRIPT;

const THREE_ROUND_SCRIPT: &str = r#"[{"tool_calls":[["call_cap_1","bash",{"command":"echo cap-one"}]],"finish_reason":"tool_use"},{"tokens":["ROUND-TWO-MARKER"],"tool_calls":[["call_cap_2","bash",{"command":"echo cap-two"}]],"finish_reason":"tool_use"},{"tokens":["ROUND-THREE-MARKER"],"finish_reason":"end_turn"}]"#;

const SINGLE_PASS_SCRIPT: &str =
    r#"[{"tokens":["Plain single pass."],"finish_reason":"end_turn"}]"#;

#[tokio::test]
async fn denied_call_feeds_back_and_loop_continues() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", DENY_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "refuse the scripted call").await;

    let frames = run_turn_resolving(&mut reader, &proc.socket_path, false).await;
    let types: Vec<&str> = frames.iter().filter_map(|f| f["type"].as_str()).collect();

    assert!(types.contains(&"tool_denied"), "types: {types:?}");
    assert!(!types.contains(&"tool_result"), "denied turns never execute");
    // Denied frames never carry execution measurements (Inc 8 parity).
    let denied_frame = frames
        .iter()
        .find(|f| f["type"] == "tool_denied")
        .unwrap();
    assert!(
        denied_frame.get("duration_ms").is_none(),
        "denied frame must not carry duration_ms: {denied_frame}"
    );
    // THE contract: the model still produced round-2 text after the denial.
    let end = frames.iter().find(|f| f["type"] == "stream_end").unwrap();
    assert!(end["response"]
        .as_str()
        .unwrap()
        .contains("Round two wraps up."));
    assert_eq!(end["finish_reason"], "end_turn");
    assert_eq!(end["metadata"]["inputTokens"], 30);
}

#[tokio::test]
async fn round_cap_stops_the_loop_gracefully() {
    let proc = start_test_daemon(&[
        ("BRAIN_MOCK_SCRIPTED_RESPONSES", THREE_ROUND_SCRIPT),
        ("BRAIN_TOOL_MAX_ROUNDS", "1"),
    ])
    .await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "cap me").await;

    let frames = run_turn_resolving(&mut reader, &proc.socket_path, true).await;
    let types: Vec<&str> = frames.iter().filter_map(|f| f["type"].as_str()).collect();

    assert_eq!(types.iter().filter(|t| **t == "tool_result").count(), 1);
    let end = frames.iter().find(|f| f["type"] == "stream_end").unwrap();
    assert_eq!(end["finish_reason"], "max_tool_rounds");
    assert!(!end["response"].as_str().unwrap().contains("ROUND-TWO-MARKER"));
    assert_eq!(end["metadata"]["inputTokens"], 15); // exactly one pass ran
}

#[tokio::test]
async fn plain_single_pass_wire_shape_is_unchanged() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", SINGLE_PASS_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "just talk").await;

    let frames = run_turn(&mut reader).await;
    let types: Vec<&str> = frames.iter().filter_map(|f| f["type"].as_str()).collect();

    assert_eq!(types.first(), Some(&"stream_start"));
    assert_eq!(types.last(), Some(&"finished"));
    assert!(!types.contains(&"tool_use"));
    assert!(!types.contains(&"tool_result"));
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(f["sequence"].as_u64().unwrap(), i as u64);
    }
    let end = frames.iter().find(|f| f["type"] == "stream_end").unwrap();
    assert_eq!(end["response"], "Plain single pass.");
    assert_eq!(end["finish_reason"], "end_turn");
    assert_eq!(end["metadata"]["inputTokens"], 15);
}

/// Loads the session back over UDS and returns the raw messages array from
/// the v1/session/load reply body.
async fn load_session_messages(
    socket_path: &std::path::Path,
    session_id: &str,
) -> Vec<serde_json::Value> {
    let loader = UnixStream::connect(socket_path).await.unwrap();
    let (lreader, mut lwriter) = loader.into_split();
    let mut lbuf = BufReader::new(lreader);
    send_frame(
        &mut lwriter,
        &serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 900,
            "action": "v1/session/load",
            "body": serde_json::json!({ "sessionId": session_id }).to_string()
        }),
    )
    .await;
    let reply = read_line_frame(&mut lbuf).await;
    let body: serde_json::Value = if let Some(s) = reply["body"].as_str() {
        serde_json::from_str(s).unwrap()
    } else {
        reply["body"].clone()
    };
    body["session"]["messages"]
        .as_array()
        .expect("messages array")
        .clone()
}

#[tokio::test]
async fn executed_tool_outcome_is_persisted_as_session_tool_message() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", TWO_ROUND_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "persist me").await;
    run_turn_resolving(&mut reader, &proc.socket_path, true).await;

    let messages = load_session_messages(&proc.socket_path, &session_id).await;

    // Transcript order: user prompt first, assistant completion last.
    assert_eq!(messages.first().unwrap()["role"], "user");
    assert_eq!(messages.last().unwrap()["role"], "assistant");

    // Exactly one tool event, shaped per spec §3.1.
    let tools: Vec<&serde_json::Value> =
        messages.iter().filter(|m| m["role"] == "tool").collect();
    assert_eq!(tools.len(), 1, "messages: {messages:?}");
    let env: serde_json::Value =
        serde_json::from_str(tools[0]["content"].as_str().unwrap()).unwrap();
    assert_eq!(env["type"], "tool_event");
    assert_eq!(env["v"], 1);
    assert_eq!(env["name"], "bash");
    assert_eq!(env["input"]["command"], "echo feedback-round-one");
    assert_eq!(env["outcome"], "executed");
    assert_eq!(env["is_error"], false);
    assert_eq!(env["exit_code"], 0);
    assert_eq!(env["output"], "feedback-round-one\n");
    assert!(env["duration_ms"].is_u64());
}

#[tokio::test]
async fn denied_tool_outcome_is_persisted_as_session_tool_message() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", DENY_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "refuse and persist").await;
    run_turn_resolving(&mut reader, &proc.socket_path, false).await;

    let messages = load_session_messages(&proc.socket_path, &session_id).await;

    assert_eq!(messages.first().unwrap()["role"], "user");
    assert_eq!(messages.last().unwrap()["role"], "assistant");

    let tools: Vec<&serde_json::Value> =
        messages.iter().filter(|m| m["role"] == "tool").collect();
    assert_eq!(tools.len(), 1, "messages: {messages:?}");
    let env: serde_json::Value =
        serde_json::from_str(tools[0]["content"].as_str().unwrap()).unwrap();
    assert_eq!(env["type"], "tool_event");
    assert_eq!(env["v"], 1);
    assert_eq!(env["name"], "bash");
    assert_eq!(env["input"]["command"], "echo feedback-round-one");
    assert_eq!(env["outcome"], "denied");
    // Nothing ran: no execution fields exist on denied envelopes.
    assert!(env.get("is_error").is_none());
    assert!(env.get("exit_code").is_none());
    assert!(env.get("output").is_none());
    assert!(env.get("duration_ms").is_none());
}

/// Inc 11: sends one `v1/shell/exec` request on its own short-lived
/// connection and returns the single reply frame.
async fn exec_shell_command(
    socket_path: &std::path::Path,
    session_id: &str,
    command: &str,
) -> serde_json::Value {
    let conn = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = conn.into_split();
    let mut buf = BufReader::new(reader);
    send_frame(
        &mut writer,
        &serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": "req-exec",
            "action": "v1/shell/exec",
            "payload": { "session_id": session_id, "command": command }
        }),
    )
    .await;
    read_line_frame(&mut buf).await
}

#[tokio::test]
async fn shell_exec_runs_command_and_persists_standalone_turn() {
    let proc = start_test_daemon(&[]).await;
    let (_r, _w, session_id) = open_and_create_session(&proc.socket_path).await;

    let reply = exec_shell_command(&proc.socket_path, &session_id, "echo bang-inc11").await;

    assert_eq!(reply["status"], "success");
    let body = &reply["body"];
    assert_eq!(body["name"], "bash");
    assert_eq!(body["input"]["command"], "echo bang-inc11");
    assert_eq!(body["outcome"], "executed");
    assert_eq!(body["output"], "bang-inc11\n");
    assert_eq!(body["is_error"], false);
    assert_eq!(body["exit_code"], 0);
    assert!(body["duration_ms"].is_u64());

    // Standalone turn persisted: user line (verbatim, with `!`) + envelope.
    let messages = load_session_messages(&proc.socket_path, &session_id).await;
    assert_eq!(messages.len(), 2, "messages: {messages:?}");
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], "! echo bang-inc11");
    assert_eq!(messages[1]["role"], "tool");
    let env: serde_json::Value =
        serde_json::from_str(messages[1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(env["type"], "tool_event");
    assert_eq!(env["v"], 1);
    assert_eq!(env["name"], "bash");
    assert_eq!(env["input"]["command"], "echo bang-inc11");
    assert_eq!(env["outcome"], "executed");
    assert_eq!(env["exit_code"], 0);
    assert_eq!(env["output"], "bang-inc11\n");
    assert!(env["duration_ms"].is_u64());
}

#[tokio::test]
async fn shell_exec_nonzero_exit_is_success_with_error_fields() {
    let proc = start_test_daemon(&[]).await;
    let (_r, _w, session_id) = open_and_create_session(&proc.socket_path).await;

    // Real BashTool runs `/bin/bash -c false` -> exit 1.
    let reply = exec_shell_command(&proc.socket_path, &session_id, "false").await;

    assert_eq!(reply["status"], "success");
    assert_eq!(reply["body"]["exit_code"], 1);
    assert_eq!(reply["body"]["is_error"], true);

    let messages = load_session_messages(&proc.socket_path, &session_id).await;
    let env: serde_json::Value =
        serde_json::from_str(messages[1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(env["outcome"], "executed");
    assert_eq!(env["exit_code"], 1);
    assert_eq!(env["is_error"], true);
}

#[tokio::test]
async fn shell_exec_rejects_empty_command_without_touching_the_transcript() {
    let proc = start_test_daemon(&[]).await;
    let (_r, _w, session_id) = open_and_create_session(&proc.socket_path).await;

    let reply = exec_shell_command(&proc.socket_path, &session_id, "   ").await;

    assert_eq!(reply["status"], "error");
    let messages = load_session_messages(&proc.socket_path, &session_id).await;
    assert_eq!(messages.len(), 0, "nothing persisted: {messages:?}");
}

#[tokio::test]
async fn shell_exec_rejects_unknown_session() {
    let proc = start_test_daemon(&[]).await;
    let reply = exec_shell_command(&proc.socket_path, "no-such-session", "echo hi").await;
    assert_eq!(reply["status"], "error");
}

#[tokio::test]
async fn shell_exec_rejects_while_a_generation_is_active() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", TWO_ROUND_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "occupying").await;
    // The generation registers in the active-generation map BEFORE its first
    // wire frame, so one received frame proves the entry exists.
    let _first =
        tokio::time::timeout(Duration::from_secs(15), read_line_frame(&mut reader))
            .await
            .expect("first stream frame");
    let reply = exec_shell_command(&proc.socket_path, &session_id, "echo nope").await;
    assert_eq!(reply["status"], "error");
    // Let the generation finish cleanly so process drop isn't mid-write.
    run_turn_resolving(&mut reader, &proc.socket_path, true).await;
}
