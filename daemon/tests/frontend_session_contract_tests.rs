use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

struct TestDaemon {
    child: Child,
    directory: PathBuf,
    socket: PathBuf,
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.directory);
    }
}

async fn start_daemon() -> TestDaemon {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let sub = format!("bfs-{}", &uuid_str[0..8]);
    let directory = {
        let tmp = std::env::temp_dir().join(&sub);
        if fs::create_dir_all(&tmp).is_err() {
            let alt = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../target/tmp")
                .join(&sub);
            fs::create_dir_all(&alt).unwrap();
            alt
        } else {
            tmp
        }
    };
    let socket = directory.join("brain.sock");
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut child = Command::new(env!("CARGO_BIN_EXE_brain-daemon"))
        .args(["daemon", "run"])
        .env("BRAIN_SOCKET_PATH", &socket)
        .env("BRAIN_PID_PATH", directory.join("brain.pid"))
        .env("BRAIN_DB_PATH", directory.join("brain.db"))
        .env("BRAIN_ANALYTICS_DB_PATH", directory.join("analytics.db"))
        .env("BRAIN_CONFIG_DIR", &directory)
        .env("BRAIN_HEALTH_PORT", port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    for _ in 0..60 {
        if UnixStream::connect(&socket).await.is_ok() {
            return TestDaemon {
                child,
                directory,
                socket,
            };
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&directory);
    panic!("test daemon did not bind its UDS socket");
}

async fn rpc(socket: &PathBuf, action: &str, payload: serde_json::Value) -> serde_json::Value {
    let stream = UnixStream::connect(socket).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let request = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": "frontend-session-contract",
        "action": action,
        "payload": payload,
    });
    writer
        .write_all(format!("{}\n", serde_json::to_string(&request).unwrap()).as_bytes())
        .await
        .unwrap();
    writer.flush().await.unwrap();
    let mut line = String::new();
    BufReader::new(reader).read_line(&mut line).await.unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

#[tokio::test]
async fn frontend_session_load_uses_typed_not_found_error() {
    let daemon = start_daemon().await;
    let response = rpc(
        &daemon.socket,
        "v1/session/load",
        serde_json::json!({ "session_id": brain_domain::SessionId::new().to_string() }),
    )
    .await;

    assert_eq!(response["status"], "error");
    assert_eq!(response["code"], "session_not_found");
}

#[tokio::test]
async fn frontend_session_start_preserves_an_existing_authoritative_session() {
    let daemon = start_daemon().await;

    let first = rpc(
        &daemon.socket,
        "v1/session/start",
        serde_json::json!({
            "title": "Authoritative original title",
            "system_prompt": "original system prompt"
        }),
    )
    .await;
    assert_eq!(first["status"], "success");
    let first_body: serde_json::Value =
        serde_json::from_str(first["body"].as_str().unwrap()).unwrap();
    let session_id = first_body["session_id"].as_str().unwrap();

    let second = rpc(
        &daemon.socket,
        "v1/session/start",
        serde_json::json!({
            "session_id": session_id,
            "title": "must not overwrite",
            "system_prompt": "must not overwrite"
        }),
    )
    .await;
    assert_eq!(second["status"], "success");

    let loaded = rpc(
        &daemon.socket,
        "v1/session/load",
        serde_json::json!({ "session_id": session_id }),
    )
    .await;
    let body: serde_json::Value = serde_json::from_str(loaded["body"].as_str().unwrap()).unwrap();
    assert_eq!(body["session"]["sessionId"], session_id);
    assert_eq!(body["session"]["title"], "Authoritative original title");
    assert_eq!(body["session"]["systemPrompt"], "original system prompt");
}
