//! Inc 26: UDS integration tests for session listing with pinned/archived truthfulness.
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
    let sub = format!("bd-session-list-{}", &uuid_str[0..8]);
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
    let db_path = test_dir.join("brain_runtime.db");
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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

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
        db_path,
    }
}

async fn rpc(
    socket_path: &PathBuf,
    id: u64,
    action: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": id,
        "action": action,
        "body": serde_json::to_string(&body).unwrap()
    });
    let mut j = serde_json::to_string(&req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    let frame: serde_json::Value = serde_json::from_str(&line).unwrap();
    if let Some(s) = frame["body"].as_str() {
        serde_json::from_str(s).unwrap_or(frame["body"].clone())
    } else {
        frame["body"].clone()
    }
}

#[tokio::test]
async fn test_uds_session_list_returns_real_pinned_and_archived_flags() {
    let daemon = start_test_daemon().await;

    // 1. Create three sessions via RPC
    let res1 = rpc(
        &daemon.socket_path,
        1,
        "v1/session/start",
        serde_json::json!({ "title": "Alpha Pinned Session" }),
    )
    .await;
    let sid_pinned = res1["session_id"]
        .as_str()
        .expect("session_id string")
        .to_string();

    let res2 = rpc(
        &daemon.socket_path,
        2,
        "v1/session/start",
        serde_json::json!({ "title": "Beta Archived Session" }),
    )
    .await;
    let sid_archived = res2["session_id"]
        .as_str()
        .expect("session_id string")
        .to_string();

    let res3 = rpc(
        &daemon.socket_path,
        3,
        "v1/session/start",
        serde_json::json!({ "title": "Gamma Standard Session" }),
    )
    .await;
    let sid_standard = res3["session_id"]
        .as_str()
        .expect("session_id string")
        .to_string();

    // 2. Mutate storage to set pinned=true on sid_pinned and archived=true on sid_archived
    {
        let conn = rusqlite::Connection::open(&daemon.db_path).unwrap();

        // Update sid_pinned
        let history_pinned: String = conn
            .query_row(
                "SELECT history FROM sessions WHERE id = ?1",
                [&sid_pinned],
                |r| r.get(0),
            )
            .unwrap();
        let mut s_pinned_val: serde_json::Value = serde_json::from_str(&history_pinned).unwrap();
        s_pinned_val["pinned"] = serde_json::json!(true);
        conn.execute(
            "UPDATE sessions SET history = ?1 WHERE id = ?2",
            [
                serde_json::to_string(&s_pinned_val).unwrap(),
                sid_pinned.clone(),
            ],
        )
        .unwrap();

        // Update sid_archived
        let history_archived: String = conn
            .query_row(
                "SELECT history FROM sessions WHERE id = ?1",
                [&sid_archived],
                |r| r.get(0),
            )
            .unwrap();
        let mut s_archived_val: serde_json::Value =
            serde_json::from_str(&history_archived).unwrap();
        s_archived_val["archived"] = serde_json::json!(true);
        conn.execute(
            "UPDATE sessions SET history = ?1 WHERE id = ?2",
            [
                serde_json::to_string(&s_archived_val).unwrap(),
                sid_archived.clone(),
            ],
        )
        .unwrap();
    }

    // 3. Query v1/session/list over UDS
    let body = rpc(
        &daemon.socket_path,
        4,
        "v1/session/list",
        serde_json::json!({}),
    )
    .await;
    let sessions = body["sessions"].as_array().expect("sessions array");
    assert_eq!(sessions.len(), 3);

    let pinned_sess = sessions
        .iter()
        .find(|s| s["sessionId"] == sid_pinned)
        .unwrap();
    assert_eq!(pinned_sess["title"], "Alpha Pinned Session");
    assert_eq!(pinned_sess["pinned"], true);
    assert_eq!(pinned_sess["archived"], false);

    let archived_sess = sessions
        .iter()
        .find(|s| s["sessionId"] == sid_archived)
        .unwrap();
    assert_eq!(archived_sess["title"], "Beta Archived Session");
    assert_eq!(archived_sess["pinned"], false);
    assert_eq!(archived_sess["archived"], true);

    let standard_sess = sessions
        .iter()
        .find(|s| s["sessionId"] == sid_standard)
        .unwrap();
    assert_eq!(standard_sess["title"], "Gamma Standard Session");
    assert_eq!(standard_sess["pinned"], false);
    assert_eq!(standard_sess["archived"], false);
}
