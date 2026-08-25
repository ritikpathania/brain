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
    let path = PathBuf::from(format!("/tmp/bd-prod-{}", &uuid_str[0..8]));
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
) -> (String, serde_json::Value) {
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
            } else if frame["type"] == "finished" {
                break;
            }
        }
    }

    (accumulated, start_frame)
}

#[tokio::test]
async fn test_product_multi_turn_generation_persistence() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Multi-Turn Product Session").await;

    // Turn 1
    let (_resp1, start1) =
        execute_generation_turn(&daemon.socket_path, &session_id, "Turn 1: Introduce topic").await;
    assert_eq!(start1["type"], "stream_start");

    // Turn 2
    let (_resp2, start2) = execute_generation_turn(
        &daemon.socket_path,
        &session_id,
        "Turn 2: Follow up question",
    )
    .await;
    assert_eq!(start2["type"], "stream_start");

    // Turn 3
    let (_resp3, start3) =
        execute_generation_turn(&daemon.socket_path, &session_id, "Turn 3: Conclude").await;
    assert_eq!(start3["type"], "stream_start");

    // Verify all 9 messages (3 turns × user/thinking/assistant) in order
    let messages = load_session_messages(&daemon.socket_path, &session_id).await;
    assert_eq!(messages.len(), 9, "Expected 9 messages across 3 turns");
    for (i, role) in [
        "user", "thinking", "assistant", "user", "thinking", "assistant", "user", "thinking",
        "assistant",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(messages[i]["role"], *role);
    }
}

#[tokio::test]
async fn test_product_e2e_persistence_and_daemon_restart() {
    let mut daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Restart Persistence Session").await;

    // Turn 1 before restart
    let (_resp1, _) = execute_generation_turn(
        &daemon.socket_path,
        &session_id,
        "Important fact: Auth proxy port is 9090",
    )
    .await;

    let msgs_before = load_session_messages(&daemon.socket_path, &session_id).await;
    assert_eq!(msgs_before.len(), 3); // user + thinking + assistant

    // Hard restart daemon on existing DB
    let _ = daemon.child.kill();
    let _ = daemon.child.wait();
    restart_daemon_with_existing_db(&mut daemon).await;

    // Verify session messages intact after restart
    let msgs_after_restart = load_session_messages(&daemon.socket_path, &session_id).await;
    assert_eq!(
        msgs_after_restart.len(),
        3,
        "Session history was lost across restart!"
    );
    assert_eq!(
        msgs_after_restart[0]["content"],
        "Important fact: Auth proxy port is 9090"
    );

    // Turn 2 after restart
    let (_resp2, start2) = execute_generation_turn(
        &daemon.socket_path,
        &session_id,
        "What was the auth proxy port?",
    )
    .await;
    assert_eq!(start2["type"], "stream_start");

    let msgs_final = load_session_messages(&daemon.socket_path, &session_id).await;
    assert_eq!(
        msgs_final.len(),
        6,
        "Post-restart generation turn failed to persist"
    );
}

#[tokio::test]
async fn test_product_workspace_switching_isolation() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Workspace Switch Session").await;

    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    // Retrieve scoped to workspace-alpha
    let req_alpha = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 11,
        "action": "v1/context/retrieve",
        "body": serde_json::json!({
            "session_id": session_id,
            "query": "deployment target",
            "workspace_id": "workspace-alpha",
            "limit": 5
        }).to_string()
    });
    let mut j_alpha = serde_json::to_string(&req_alpha).unwrap();
    j_alpha.push('\n');
    writer.write_all(j_alpha.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut line_alpha = String::new();
    buf_reader.read_line(&mut line_alpha).await.unwrap();
    let frame_alpha: serde_json::Value = serde_json::from_str(&line_alpha).unwrap();
    assert_eq!(frame_alpha["status"], "success");

    // Switch to workspace-beta
    let req_beta = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 12,
        "action": "v1/context/retrieve",
        "body": serde_json::json!({
            "session_id": session_id,
            "query": "deployment target",
            "workspace_id": "workspace-beta",
            "limit": 5
        }).to_string()
    });
    let mut j_beta = serde_json::to_string(&req_beta).unwrap();
    j_beta.push('\n');
    writer.write_all(j_beta.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut line_beta = String::new();
    buf_reader.read_line(&mut line_beta).await.unwrap();
    let frame_beta: serde_json::Value = serde_json::from_str(&line_beta).unwrap();
    assert_eq!(frame_beta["status"], "success");
    let body_beta: serde_json::Value =
        serde_json::from_str(frame_beta["body"].as_str().unwrap()).unwrap();

    // Verify zero cross-workspace contamination
    for m in body_beta["memories"].as_array().unwrap() {
        let scope = m["scope"].as_str().unwrap();
        assert!(!scope.contains("workspace:workspace-alpha"));
    }
}

#[tokio::test]
async fn test_product_session_forking_with_context_continuity() {
    let daemon = start_test_daemon().await;
    let parent_id = create_test_session(&daemon.socket_path, "Parent Session").await;

    // Parent Turn 1
    let (_resp1, _) =
        execute_generation_turn(&daemon.socket_path, &parent_id, "Parent message 1").await;

    // Fork Parent session
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let fork_req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 20,
        "action": "v1/session/fork",
        "body": serde_json::json!({
            "source_session_id": parent_id,
            "new_title": "Forked Child Session"
        }).to_string()
    });
    let mut fork_json = serde_json::to_string(&fork_req).unwrap();
    fork_json.push('\n');
    writer.write_all(fork_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut fork_resp_line = String::new();
    buf_reader.read_line(&mut fork_resp_line).await.unwrap();
    let fork_frame: serde_json::Value = serde_json::from_str(&fork_resp_line).unwrap();
    let fork_body: serde_json::Value =
        serde_json::from_str(fork_frame["body"].as_str().unwrap()).unwrap();
    let child_id = fork_body["new_session_id"].as_str().unwrap().to_string();

    // Verify child has cloned messages (user + thinking + assistant)
    let child_msgs = load_session_messages(&daemon.socket_path, &child_id).await;
    assert_eq!(child_msgs.len(), 3);

    // Child Turn 2 executes independently
    let (_resp2, start2) =
        execute_generation_turn(&daemon.socket_path, &child_id, "Child turn 2").await;
    assert_eq!(start2["type"], "stream_start");

    // Verify parent has 3 messages, child has 6 messages
    let parent_final = load_session_messages(&daemon.socket_path, &parent_id).await;
    let child_final = load_session_messages(&daemon.socket_path, &child_id).await;
    assert_eq!(
        parent_final.len(),
        3,
        "Parent session was mutated by child!"
    );
    assert_eq!(
        child_final.len(),
        6,
        "Child session failed to branch independently"
    );
}

#[tokio::test]
async fn test_adversarial_high_concurrency_indexing_and_generation() {
    let daemon = start_test_daemon().await;
    let socket_path = daemon.socket_path.clone();

    // Spawn 5 distinct sessions
    let mut session_ids = Vec::new();
    for i in 0..5 {
        let sid = create_test_session(&socket_path, &format!("Concurrent Session {}", i)).await;
        session_ids.push(sid);
    }

    let mut handles = Vec::new();

    // 5 concurrent generation streams
    for (i, sid) in session_ids.iter().enumerate() {
        let sp = socket_path.clone();
        let s = sid.clone();
        let handle = tokio::spawn(async move {
            let (resp, start) =
                execute_generation_turn(&sp, &s, &format!("Concurrent Turn {}", i)).await;
            assert_eq!(start["type"], "stream_start");
            assert!(!resp.is_empty());
        });
        handles.push(handle);
    }

    // 10 concurrent session operations (simulating ingestion WAL activity)
    for i in 0..10 {
        let sp = socket_path.clone();
        let handle = tokio::spawn(async move {
            let stream = UnixStream::connect(&sp).await.unwrap();
            let (reader, mut writer) = stream.into_split();
            let mut buf_reader = BufReader::new(reader);

            let req = serde_json::json!({
                "version": "1.0",
                "type": "Request",
                "id": 100 + i,
                "action": "v1/session/create",
                "body": serde_json::json!({
                    "title": format!("Background Ingest Batch {}", i)
                }).to_string()
            });
            let mut j = serde_json::to_string(&req).unwrap();
            j.push('\n');
            writer.write_all(j.as_bytes()).await.unwrap();
            writer.flush().await.unwrap();

            let mut line = String::new();
            buf_reader.read_line(&mut line).await.unwrap();
            let frame: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(frame["status"], "success");
        });
        handles.push(handle);
    }

    // Wait for all concurrent generation and indexing tasks to finish
    for h in handles {
        h.await.unwrap();
    }

    // Assert every generation session completed with exactly 3 messages
    // (1 user, 1 thinking, 1 assistant)
    for sid in &session_ids {
        let msgs = load_session_messages(&socket_path, sid).await;
        assert_eq!(
            msgs.len(),
            3,
            "Concurrency corruption: session '{}' missing completed turn",
            sid
        );
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[1]["role"], "thinking");
        assert_eq!(msgs[2]["role"], "assistant");
    }
}

#[tokio::test]
async fn test_product_session_archival_and_restoration_gating() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Archival Test Session").await;

    // Execute Turn 1
    let (_resp1, _) =
        execute_generation_turn(&daemon.socket_path, &session_id, "Turn 1 before archive").await;

    // Archive session via RPC
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let archive_req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 50,
        "action": "v1/session/archive",
        "body": serde_json::json!({
            "session_id": session_id
        }).to_string()
    });
    let mut arc_json = serde_json::to_string(&archive_req).unwrap();
    arc_json.push('\n');
    writer.write_all(arc_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut arc_resp_line = String::new();
    buf_reader.read_line(&mut arc_resp_line).await.unwrap();
    let arc_frame: serde_json::Value = serde_json::from_str(&arc_resp_line).unwrap();
    assert_eq!(arc_frame["status"], "success");

    // Restore session via RPC
    let restore_req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 51,
        "action": "v1/session/restore",
        "body": serde_json::json!({
            "session_id": session_id
        }).to_string()
    });
    let mut res_json = serde_json::to_string(&restore_req).unwrap();
    res_json.push('\n');
    writer.write_all(res_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut res_resp_line = String::new();
    buf_reader.read_line(&mut res_resp_line).await.unwrap();
    let res_frame: serde_json::Value = serde_json::from_str(&res_resp_line).unwrap();
    assert_eq!(res_frame["status"], "success");

    // Post-restoration generation turn executes cleanly
    let (_resp2, start2) =
        execute_generation_turn(&daemon.socket_path, &session_id, "Turn 2 after restore").await;
    assert_eq!(start2["type"], "stream_start");

    let final_msgs = load_session_messages(&daemon.socket_path, &session_id).await;
    // 2 turns × (user + thinking + assistant) accumulated across the
    // archive/restore boundary
    assert_eq!(
        final_msgs.len(),
        6,
        "Restored session failed to accumulate turns"
    );
}
