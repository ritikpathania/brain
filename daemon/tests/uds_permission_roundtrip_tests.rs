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
