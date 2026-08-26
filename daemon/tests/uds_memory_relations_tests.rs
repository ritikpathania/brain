//! Memory relations round-trip over UDS: what memory/store persists,
//! memory/search must return — including relations, content-derived
//! excerpts, and stored scope.

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
    pid_path: PathBuf,
    db_path: PathBuf,
    analytics_db_path: PathBuf,
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
    let path = PathBuf::from(format!("/tmp/bd-memrel-{}", &uuid_str[0..8]));
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

fn spawn_daemon(test_dir: &PathBuf, socket: &PathBuf, pid: &PathBuf, db: &PathBuf, analytics: &PathBuf) -> Child {
    Command::new(env!("CARGO_BIN_EXE_brain-daemon"))
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", socket)
        .env("BRAIN_PID_PATH", pid)
        .env("BRAIN_DB_PATH", db)
        .env("BRAIN_ANALYTICS_DB_PATH", analytics)
        .env("BRAIN_CONFIG_DIR", test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon process")
}

async fn start_daemon_at(test_dir: PathBuf) -> DaemonProcess {
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");
    let child = spawn_daemon(&test_dir, &socket_path, &pid_path, &db_path, &analytics_db_path);
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
        pid_path,
        db_path,
        analytics_db_path,
    }
}

/// One versioned RPC round-trip; returns the parsed response BODY.
async fn rpc(socket_path: &PathBuf, id: u64, action: &str, body: serde_json::Value) -> serde_json::Value {
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
        serde_json::from_str(s).unwrap()
    } else {
        frame["body"].clone()
    }
}

#[tokio::test]
async fn stored_relations_round_trip_through_search() {
    let d = start_daemon_at(get_temp_dir()).await;

    let stored = rpc(
        &d.socket_path,
        1,
        "memory/store",
        serde_json::json!({
            "label": "Alpha Cortex Node",
            "content": "Cortex excerpt body for the smoke",
            "scope": "workspace",
            "relations": [
                {"relation": "supports", "target_id": "beta-1", "target_label": "Beta Concept"}
            ],
        }),
    )
    .await;
    assert_eq!(stored["success"], true, "store failed: {stored}");

    let found = rpc(
        &d.socket_path,
        2,
        "memory/search",
        serde_json::json!({"query": "cortex", "limit": 10}),
    )
    .await;
    let memories = found["memories"]
        .as_array()
        .expect("memories array in response body")
        .clone();
    assert!(memories.len() >= 1, "seeded node must be returned: {found}");
    let first = &memories[0];
    assert_eq!(first["label"], "Alpha Cortex Node");
    // Excerpt must be the STORED CONTENT, not the echoed label.
    assert_eq!(first["excerpt"], "Cortex excerpt body for the smoke");
    assert_eq!(first["scope"], "workspace");
    let rels = first["relations"].as_array().expect("relations array").clone();
    assert_eq!(rels.len(), 1, "stored relation must round-trip: {first}");
    assert_eq!(rels[0]["relation"], "supports");
    assert_eq!(rels[0]["target_id"], "beta-1");
    assert_eq!(rels[0]["target_label"], "Beta Concept");
}

#[tokio::test]
async fn store_without_relations_yields_empty_relation_list() {
    let d = start_daemon_at(get_temp_dir()).await;

    let stored = rpc(
        &d.socket_path,
        1,
        "memory/store",
        serde_json::json!({"label": "Plain Node", "content": "plain body"}),
    )
    .await;
    assert_eq!(stored["success"], true);

    let found = rpc(
        &d.socket_path,
        2,
        "memory/search",
        serde_json::json!({"query": "plain", "limit": 5}),
    )
    .await;
    let memories = found["memories"].as_array().expect("memories array");
    assert!(memories.len() >= 1);
    assert_eq!(
        memories[0]["relations"].as_array().expect("relations key").len(),
        0,
        "absent relations must surface as an empty list"
    );
}

#[tokio::test]
async fn blank_query_lists_stored_concepts_newest_first() {
    let d = start_daemon_at(get_temp_dir()).await;

    rpc(
        &d.socket_path,
        1,
        "memory/store",
        serde_json::json!({
            "label": "Older Plain Node",
            "content": "older body",
        }),
    )
    .await;

    rpc(
        &d.socket_path,
        2,
        "memory/store",
        serde_json::json!({
            "label": "Newer Related Node",
            "content": "newer body",
            "scope": "compiler",
            "relations": [{"relation": "supports", "target_id": "beta-1"}],
        }),
    )
    .await;

    let found = rpc(
        &d.socket_path,
        3,
        "memory/search",
        serde_json::json!({"query": "", "limit": 10}),
    )
    .await;
    let m = found["memories"].as_array().expect("memories array");
    assert_eq!(m.len(), 2, "both stored concepts listed: {found}");
    assert_eq!(m[0]["label"], "Newer Related Node", "newest first");
    assert_eq!(m[0]["excerpt"], "newer body");
    assert_eq!(m[0]["scope"], "compiler");
    assert_eq!(m[0]["relations"][0]["relation"], "supports");
    assert_eq!(m[1]["label"], "Older Plain Node");
    assert_eq!(m[1]["relations"].as_array().unwrap().len(), 0);

    let ws = rpc(
        &d.socket_path,
        4,
        "memory/search",
        serde_json::json!({"query": "   ", "limit": 10}),
    )
    .await;
    assert_eq!(
        ws["memories"].as_array().unwrap().len(),
        2,
        "whitespace-only query behaves as blank"
    );
}

#[tokio::test]
async fn blank_query_honors_limit_and_typed_queries_unchanged() {
    let d = start_daemon_at(get_temp_dir()).await;

    for i in 1..=3 {
        rpc(
            &d.socket_path,
            i,
            "memory/store",
            serde_json::json!({"label": format!("Node{i}"), "content": "c"}),
        )
        .await;
    }

    let limited = rpc(
        &d.socket_path,
        10,
        "memory/search",
        serde_json::json!({"query": "", "limit": 2}),
    )
    .await;
    assert_eq!(
        limited["memories"].as_array().unwrap().len(),
        2,
        "blank listing honors limit: {limited}"
    );

    let typed = rpc(
        &d.socket_path,
        11,
        "memory/search",
        serde_json::json!({"query": "Node2", "limit": 10}),
    )
    .await;
    let tm = typed["memories"].as_array().unwrap();
    assert!(
        tm.iter().any(|x| x["label"] == "Node2"),
        "single-token typed query still finds the stored node: {typed}"
    );
}
