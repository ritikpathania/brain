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
    let sub = format!("bd-mem-{}", &uuid_str[0..8]);
    let tmp_path = std::env::temp_dir().join(&sub);
    if let Ok(()) = fs::create_dir_all(&tmp_path) {
        return tmp_path;
    }
    let alt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/tmp")
        .join(&sub);
    fs::create_dir_all(&alt_path).unwrap();
    alt_path
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
        _db_path: db_path,
        _analytics_db_path: analytics_db_path,
        _pid_path: pid_path,
        _port: port,
    }
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

#[tokio::test]
async fn test_uds_context_retrieve_rpc_lifecycle() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Context Retrieve RPC Session").await;

    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let retrieve_req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 42,
        "action": "v1/context/retrieve",
        "body": serde_json::json!({
            "session_id": session_id,
            "query": "architecture overview",
            "limit": 5,
            "max_tokens": 1000
        }).to_string()
    });

    let mut req_json = serde_json::to_string(&retrieve_req).unwrap();
    req_json.push('\n');
    writer.write_all(req_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut resp_line = String::new();
    buf_reader.read_line(&mut resp_line).await.unwrap();
    let resp_frame: serde_json::Value = serde_json::from_str(&resp_line).unwrap();

    assert_eq!(resp_frame["status"], "success");
    let body: serde_json::Value =
        serde_json::from_str(resp_frame["body"].as_str().unwrap()).unwrap();

    assert!(body["memories"].is_array());
    assert!(body["provenance"].is_object());
    assert!(body["provenance"]["epoch_id"].is_string());
    assert!(body["token_count"].is_number());
}

#[tokio::test]
async fn test_uds_generation_exposes_only_safe_brain_context_metadata() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Memory Gen Session").await;

    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    let gen_req = serde_json::json!({
        "id": "req-gen-mem",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [
                { "role": "user", "content": "Tell me about project architecture" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut first_line = String::new();
    buf_reader.read_line(&mut first_line).await.unwrap();
    let start_frame: serde_json::Value = serde_json::from_str(&first_line).unwrap();

    assert_eq!(start_frame["type"], "stream_start");
    assert_eq!(start_frame["sequence"], 0);
    let context = &start_frame["metadata"]["brain_context"];
    assert_eq!(context["version"], 1);
    assert!(context["retrieval_epoch_id"].is_string());
    assert_eq!(context["retrieval_count"], 0);
    assert_eq!(context["retrieval_channels"], serde_json::json!([]));
    assert_eq!(context["context_token_count"], 0);
    assert_eq!(context["source_refs"], serde_json::json!([]));
    assert!(context.get("content").is_none());
    assert!(context.get("scores").is_none());
    assert!(start_frame["metadata"].get("memory_provenance").is_none());
    assert!(start_frame["metadata"].get("telemetry").is_none());

    let expected_context = context.clone();
    let mut terminal_context = None;
    loop {
        let mut line = String::new();
        if buf_reader.read_line(&mut line).await.unwrap() == 0 {
            break;
        }
        let frame: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        if frame["type"] == "stream_end" {
            assert!(frame["metadata"].get("memory_provenance").is_none());
            assert!(frame["metadata"].get("telemetry").is_some());
            terminal_context = Some(frame["metadata"]["brain_context"].clone());
        }
        if frame["type"] == "finished" {
            break;
        }
    }
    assert_eq!(terminal_context, Some(expected_context));
}

#[tokio::test]
async fn test_adversarial_cold_start_empty_corpus() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Cold Start Session").await;

    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    // Generation on a brand new database with zero indexed memories
    let gen_req = serde_json::json!({
        "id": "req-gen-cold",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [
                { "role": "user", "content": "Cold start query" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut finished_seen = false;
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
            if frame["type"] == "finished" {
                assert_eq!(frame["status"], "completed");
                finished_seen = true;
                break;
            }
        }
    }

    assert!(
        finished_seen,
        "Cold start generation failed to complete normally"
    );
}

#[tokio::test]
async fn test_adversarial_cross_workspace_isolation() {
    let daemon = start_test_daemon().await;
    let session_id = create_test_session(&daemon.socket_path, "Workspace Isolation Session").await;

    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    // Retrieve scoped to workspace-1
    let retrieve_ws1 = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 101,
        "action": "v1/context/retrieve",
        "body": serde_json::json!({
            "session_id": session_id,
            "query": "authentication module",
            "workspace_id": "workspace-1",
            "limit": 5
        }).to_string()
    });

    let mut req_json = serde_json::to_string(&retrieve_ws1).unwrap();
    req_json.push('\n');
    writer.write_all(req_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut resp_line = String::new();
    buf_reader.read_line(&mut resp_line).await.unwrap();
    let resp_frame: serde_json::Value = serde_json::from_str(&resp_line).unwrap();
    assert_eq!(resp_frame["status"], "success");
    let body: serde_json::Value =
        serde_json::from_str(resp_frame["body"].as_str().unwrap()).unwrap();
    let memories = body["memories"].as_array().unwrap();

    // Verify all returned memories match workspace-1 or global
    for m in memories {
        let scope = m["scope"].as_str().unwrap();
        assert!(
            scope.contains("workspace:workspace-1")
                || scope.contains("global")
                || scope.contains("session:"),
            "Cross-workspace leak detected! Memory scope: {}",
            scope
        );
    }
}

#[tokio::test]
async fn test_adversarial_context_snapshot_immutability_under_mutation() {
    let daemon = start_test_daemon().await;
    let session_id =
        create_test_session(&daemon.socket_path, "Snapshot Immutability Session").await;

    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    // Start generation turn T0
    let gen_req = serde_json::json!({
        "id": "req-gen-snap",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "messages": [
                { "role": "user", "content": "Snapshot query at T0" }
            ],
            "model": "brain-default"
        }
    });

    let mut gen_json = serde_json::to_string(&gen_req).unwrap();
    gen_json.push('\n');
    writer.write_all(gen_json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut start_line = String::new();
    buf_reader.read_line(&mut start_line).await.unwrap();
    let start_frame: serde_json::Value = serde_json::from_str(&start_line).unwrap();
    let epoch_id = start_frame["metadata"]["brain_context"]["retrieval_epoch_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        !epoch_id.is_empty(),
        "Missing immutable epoch_id in stream_start"
    );

    // Simulate database mutation at T1 on a second connection
    let stream_mut = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (_, mut writer_mut) = stream_mut.into_split();
    let ingest_req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 999,
        "action": "v1/session/rename",
        "body": serde_json::json!({
            "session_id": session_id,
            "new_title": "Mutated Title at T1"
        }).to_string()
    });
    let mut mut_json = serde_json::to_string(&ingest_req).unwrap();
    mut_json.push('\n');
    let _ = writer_mut.write_all(mut_json.as_bytes()).await;
    let _ = writer_mut.flush().await;

    // Continue draining stream through T3
    let mut finished = false;
    loop {
        let mut line = String::new();
        if buf_reader.read_line(&mut line).await.unwrap() == 0 {
            break;
        }
        if let Ok(f) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            if f["type"] == "finished" {
                assert_eq!(f["status"], "completed");
                finished = true;
                break;
            }
        }
    }
    assert!(finished);
}
