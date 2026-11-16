//! Black-box contract tests for the canonical frontend handshake.
//!
//! These use the daemon binary and its newline-delimited UDS transport so the
//! wire-level compatibility boundary is tested independently of the frontend.

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
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.test_dir);
    }
}

fn temporary_test_dir() -> PathBuf {
    let sub = format!("bfh-{}", &uuid::Uuid::new_v4().to_string()[0..8]);
    let tmp_path = std::env::temp_dir().join(&sub);
    if let Ok(()) = fs::create_dir_all(&tmp_path) {
        return tmp_path;
    }
    let alt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/tmp")
        .join(&sub);
    fs::create_dir_all(&alt_path).expect("create test directory");
    alt_path
}

fn free_tcp_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read ephemeral port")
        .port()
}

async fn start_test_daemon() -> DaemonProcess {
    let test_dir = temporary_test_dir();
    let socket_path = test_dir.join("brain.sock");
    let mut child = Command::new(env!("CARGO_BIN_EXE_brain-daemon"))
        .args(["daemon", "run"])
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", test_dir.join("brain.pid"))
        .env("BRAIN_DB_PATH", test_dir.join("brain.db"))
        .env("BRAIN_ANALYTICS_DB_PATH", test_dir.join("analytics.db"))
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", free_tcp_port().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start daemon");

    for _ in 0..60 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            return DaemonProcess {
                child,
                test_dir,
                socket_path,
            };
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&test_dir);
    panic!("daemon did not bind its UDS socket");
}

fn valid_handshake_payload() -> serde_json::Value {
    serde_json::json!({
        "protocol": "brain.frontend",
        "minimumProtocolVersion": 1,
        "maximumProtocolVersion": 1,
        "frontendName": "claude-code",
        "frontendVersion": "test-frontend-version",
        "frontendBuildId": "test-build",
        "requiredCapabilities": [
            "sessions.v1",
            "generation.stream.v1",
            "generation.cancel.v1"
        ],
        "optionalCapabilities": ["memory.provenance.v1"]
    })
}

async fn request(
    socket_path: &std::path::Path,
    action: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    let stream = UnixStream::connect(socket_path).await.expect("connect UDS");
    let (reader, mut writer) = stream.into_split();
    let frame = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": 42,
        "action": action,
        "body": body.to_string()
    });
    writer
        .write_all(format!("{}\n", frame).as_bytes())
        .await
        .expect("write frontend handshake request");
    writer
        .flush()
        .await
        .expect("flush frontend handshake request");

    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut line))
        .await
        .expect("handshake response timeout")
        .expect("read handshake response");
    serde_json::from_str(line.trim()).expect("parse handshake response")
}

fn response_body(response: &serde_json::Value) -> serde_json::Value {
    let body = response["body"]
        .as_str()
        .expect("versioned frontend handshake body is a JSON string");
    serde_json::from_str(body).expect("parse frontend handshake body")
}

#[tokio::test]
async fn compatible_frontend_handshake_selects_v1_and_authoritative_backend_metadata() {
    let daemon = start_test_daemon().await;

    let response = request(
        &daemon.socket_path,
        "v1/frontend/handshake",
        valid_handshake_payload(),
    )
    .await;

    assert_eq!(response["type"], "Response", "{response}");
    assert_eq!(response["status"], "success", "{response}");
    let body = response_body(&response);
    assert_eq!(body["selectedProtocolVersion"], 1, "{body}");
    assert_eq!(body["backendName"], "brain-daemon", "{body}");
    assert_eq!(body["backendVersion"], env!("CARGO_PKG_VERSION"), "{body}");
    assert_eq!(body["daemonState"], "ready", "{body}");
    assert_eq!(
        body["supportedCapabilities"],
        serde_json::json!([
            "generation.cancel.v1",
            "generation.stream.v1",
            "sessions.v1"
        ]),
        "{body}"
    );
    assert_eq!(
        body["acceptedOptionalCapabilities"],
        serde_json::json!([]),
        "{body}"
    );
}

#[tokio::test]
async fn incompatible_protocol_range_returns_typed_incompatibility() {
    let daemon = start_test_daemon().await;
    let mut payload = valid_handshake_payload();
    payload["minimumProtocolVersion"] = serde_json::json!(2);
    payload["maximumProtocolVersion"] = serde_json::json!(2);

    let response = request(&daemon.socket_path, "v1/frontend/handshake", payload).await;

    assert_eq!(response["type"], "Error", "{response}");
    let body = response_body(&response);
    assert_eq!(body["code"], "unsupported_protocol_version", "{body}");
    assert_eq!(body["supportedMinimumProtocolVersion"], 1, "{body}");
    assert_eq!(body["supportedMaximumProtocolVersion"], 1, "{body}");
}

#[tokio::test]
async fn missing_required_capability_fails_before_frontend_use() {
    let daemon = start_test_daemon().await;
    let mut payload = valid_handshake_payload();
    payload["requiredCapabilities"] = serde_json::json!(["sessions.v1", "tools.v9"]);

    let response = request(&daemon.socket_path, "v1/frontend/handshake", payload).await;

    assert_eq!(response["type"], "Error", "{response}");
    let body = response_body(&response);
    assert_eq!(body["code"], "missing_required_capability", "{body}");
    assert_eq!(
        body["missingRequiredCapabilities"],
        serde_json::json!(["tools.v9"]),
        "{body}"
    );
}

#[tokio::test]
async fn unavailable_optional_capability_is_not_accepted() {
    let daemon = start_test_daemon().await;

    let response = request(
        &daemon.socket_path,
        "v1/frontend/handshake",
        valid_handshake_payload(),
    )
    .await;

    let body = response_body(&response);
    assert_eq!(
        body["acceptedOptionalCapabilities"],
        serde_json::json!([]),
        "{body}"
    );
}

#[tokio::test]
async fn malformed_handshake_payload_returns_a_typed_protocol_error_and_daemon_stays_healthy() {
    let daemon = start_test_daemon().await;

    let malformed = request(
        &daemon.socket_path,
        "v1/frontend/handshake",
        serde_json::Value::String("not-an-object".to_string()),
    )
    .await;

    assert_eq!(malformed["type"], "Error", "{malformed}");
    let body = response_body(&malformed);
    assert_eq!(body["code"], "malformed_handshake_request", "{body}");

    let healthy = request(
        &daemon.socket_path,
        "v1/frontend/handshake",
        valid_handshake_payload(),
    )
    .await;
    assert_eq!(healthy["type"], "Response", "{healthy}");
}

#[tokio::test]
async fn unknown_frontend_handshake_version_has_a_stable_error_response() {
    let daemon = start_test_daemon().await;

    let response = request(
        &daemon.socket_path,
        "v2/frontend/handshake",
        valid_handshake_payload(),
    )
    .await;

    assert_eq!(response["type"], "Error", "{response}");
    assert_eq!(response["status"], "error", "{response}");
    assert!(
        response["body"]
            .as_str()
            .is_some_and(|body| body.contains("v2/frontend/handshake")),
        "{response}"
    );
}
