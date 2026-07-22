use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

fn get_temp_socket_path() -> PathBuf {
    let rand_val = rand::random::<u32>();
    std::env::temp_dir().join(format!("brain-test-cli-{}.sock", rand_val))
}

#[tokio::test]
async fn test_cli_version() {
    let bin_path = env!("CARGO_BIN_EXE_brain-cli-adapter");
    let output = Command::new(bin_path)
        .arg("version")
        .output()
        .expect("failed to execute binary");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("SDK version: 0.1.0"));
    assert!(stdout.contains("Event Model version: 1.0"));
    assert!(stdout.contains("Supported serializations: json"));
}

#[tokio::test]
async fn test_cli_ping_fail() {
    let bin_path = env!("CARGO_BIN_EXE_brain-cli-adapter");
    let socket_path = get_temp_socket_path();

    // Intentionally no listener on this socket path
    let output = Command::new(bin_path)
        .arg("--socket-path")
        .arg(&socket_path)
        .arg("ping")
        .output()
        .expect("failed to execute binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Ping failed"));
}

#[tokio::test]
async fn test_cli_ping_success() {
    let bin_path = env!("CARGO_BIN_EXE_brain-cli-adapter");
    let socket_path = get_temp_socket_path();
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path).expect("failed to bind socket");

    // Run the ping command which should connect, transition to Ready, and exit
    let socket_path_clone = socket_path.clone();
    let client_handle = tokio::task::spawn_blocking(move || {
        Command::new(bin_path)
            .arg("--socket-path")
            .arg(&socket_path_clone)
            .arg("ping")
            .output()
            .expect("failed to execute binary")
    });

    // Accept the ping connection
    let accept_res = tokio::time::timeout(Duration::from_secs(10), listener.accept()).await;
    assert!(accept_res.is_ok(), "Mock server did not receive connection");
    let (stream, _) = accept_res.unwrap().unwrap();
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();

    // Read the handshake request
    reader.read_line(&mut line).await.unwrap();
    // Respond to handshake
    let reply = serde_json::json!({
        "status": "success",
        "body": "handshake ok"
    });
    let mut reply_str = serde_json::to_string(&reply).unwrap();
    reply_str.push('\n');
    write.write_all(reply_str.as_bytes()).await.unwrap();
    write.flush().await.unwrap();

    let output = client_handle.await.expect("join failed");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ping successful"));

    let _ = std::fs::remove_file(&socket_path);
}

#[tokio::test]
async fn test_cli_send_message() {
    let bin_path = env!("CARGO_BIN_EXE_brain-cli-adapter");
    let socket_path = get_temp_socket_path();
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path).expect("failed to bind socket");

    let socket_path_clone = socket_path.clone();
    let client_handle = tokio::task::spawn_blocking(move || {
        Command::new(bin_path)
            .arg("--socket-path")
            .arg(&socket_path_clone)
            .arg("send")
            .arg("message")
            .arg("--role")
            .arg("user")
            .arg("--text")
            .arg("Hello from CLI integration test")
            .output()
            .expect("failed to execute binary")
    });

    // Handle the UDS connection in the mock server
    let (stream, _) = listener.accept().await.expect("failed to accept");
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();

    // 1. Read the handshake request
    reader
        .read_line(&mut line)
        .await
        .expect("failed to read handshake");

    // Reply with Handshake response
    let reply_hs = serde_json::json!({
        "status": "success",
        "body": "handshake ok"
    });
    let mut reply_hs_str = serde_json::to_string(&reply_hs).unwrap();
    reply_hs_str.push('\n');
    write.write_all(reply_hs_str.as_bytes()).await.unwrap();
    write.flush().await.unwrap();

    // 2. Read requests until we get the ingest_event request
    let payload_str = loop {
        line.clear();
        reader
            .read_line(&mut line)
            .await
            .expect("failed to read request");
        let request_json: serde_json::Value =
            serde_json::from_str(&line).expect("invalid JSON request");
        let action = request_json
            .get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("");
        if action == "v1/subscribe" {
            let req_id = request_json
                .get("id")
                .and_then(|id| id.as_u64())
                .unwrap_or(0);
            let reply_sub = serde_json::json!({
                "id": req_id,
                "type": "Response",
                "status": "success",
                "body": "subscription ok"
            });
            let mut reply_sub_str = serde_json::to_string(&reply_sub).unwrap();
            reply_sub_str.push('\n');
            write.write_all(reply_sub_str.as_bytes()).await.unwrap();
            write.flush().await.unwrap();
        } else {
            let p_str = request_json
                .get("body")
                .or_else(|| request_json.get("payload"))
                .and_then(|p| p.as_str())
                .unwrap();
            break p_str.to_string();
        }
    };

    let envelope: serde_json::Value = serde_json::from_str(&payload_str).unwrap();
    let event_id = envelope
        .get("identity")
        .and_then(|i| i.get("event_id"))
        .and_then(|id| id.as_str())
        .unwrap();

    // Reply with IngestAck
    let ack_body = serde_json::json!({
        "sequence": 100,
        "event_id": event_id
    });
    let reply = serde_json::json!({
        "message": serde_json::to_string(&ack_body).unwrap()
    });
    let mut reply_str = serde_json::to_string(&reply).unwrap();
    reply_str.push('\n');
    write.write_all(reply_str.as_bytes()).await.unwrap();
    write.flush().await.unwrap();

    // Verify command output
    let output = client_handle.await.expect("join failed");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Event ingested successfully."));
    assert!(stdout.contains("Sequence: 100"));

    let _ = std::fs::remove_file(&socket_path);
}
