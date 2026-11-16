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

fn spawn_daemon(
    test_dir: &PathBuf,
    socket: &PathBuf,
    pid: &PathBuf,
    db: &PathBuf,
    analytics: &PathBuf,
) -> Child {
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
    let db_path = test_dir.join("brain_runtime.db");
    let analytics_db_path = test_dir.join("analytics.db");
    let child = spawn_daemon(
        &test_dir,
        &socket_path,
        &pid_path,
        &db_path,
        &analytics_db_path,
    );
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

/// One versioned RPC round-trip; returns the parsed response BODY.
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
    assert!(
        !memories.is_empty(),
        "seeded node must be returned: {found}"
    );
    let first = &memories[0];
    assert_eq!(first["label"], "Alpha Cortex Node");
    // Excerpt must be the STORED CONTENT, not the echoed label.
    assert_eq!(first["excerpt"], "Cortex excerpt body for the smoke");
    assert_eq!(first["scope"], "workspace");
    let rels = first["relations"]
        .as_array()
        .expect("relations array")
        .clone();
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
    assert!(!memories.is_empty());
    assert_eq!(
        memories[0]["relations"]
            .as_array()
            .expect("relations key")
            .len(),
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

#[tokio::test]
async fn canonical_edges_round_trip_through_search_with_resolved_labels() {
    let d = start_daemon_at(get_temp_dir()).await;

    // 1. Store concept A and concept B via UDS
    let res_a = rpc(
        &d.socket_path,
        1,
        "v1/memory/store",
        serde_json::json!({
            "label": "Alpha Architecture Service",
            "content": "Core architecture service node",
            "scope": "workspace",
        }),
    )
    .await;
    let node_a_id = res_a["node_id"]
        .as_str()
        .expect("node_id for A")
        .to_string();

    let res_b = rpc(
        &d.socket_path,
        2,
        "v1/memory/store",
        serde_json::json!({
            "label": "Beta Dispatch Engine",
            "content": "Event dispatch engine node",
            "scope": "workspace",
        }),
    )
    .await;
    let node_b_id = res_b["node_id"]
        .as_str()
        .expect("node_id for B")
        .to_string();

    // 2. Insert canonical edge into the SQLite edges table directly
    {
        let conn = rusqlite::Connection::open(&d.db_path).unwrap();
        conn.execute(
            "INSERT INTO edges (source, target, relation, weight, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![&node_a_id, &node_b_id, "uses", 1.0, 1000i64],
        )
        .unwrap();
    }

    // 3. Query memory search for Alpha (outgoing edge)
    let search_a = rpc(
        &d.socket_path,
        3,
        "v1/memory/search",
        serde_json::json!({"query": "Alpha", "limit": 5}),
    )
    .await;
    let memories_a = search_a["memories"].as_array().expect("memories array");
    assert!(!memories_a.is_empty());
    let rels_a = memories_a[0]["relations"]
        .as_array()
        .expect("relations on Alpha");
    assert_eq!(rels_a.len(), 1);
    assert_eq!(rels_a[0]["relation"], "uses");
    assert_eq!(rels_a[0]["target_id"], node_b_id);
    assert_eq!(rels_a[0]["target_label"], "Beta Dispatch Engine");

    // 4. Query memory search for Beta (incoming edge)
    let search_b = rpc(
        &d.socket_path,
        4,
        "v1/memory/search",
        serde_json::json!({"query": "Beta", "limit": 5}),
    )
    .await;
    let memories_b = search_b["memories"].as_array().expect("memories array");
    assert!(!memories_b.is_empty());
    let rels_b = memories_b[0]["relations"]
        .as_array()
        .expect("relations on Beta");
    assert_eq!(rels_b.len(), 1);
    assert_eq!(rels_b[0]["relation"], "uses");
    assert_eq!(rels_b[0]["target_id"], node_a_id);
    assert_eq!(rels_b[0]["target_label"], "Alpha Architecture Service");

    // 5. Blank query must also materialize relations with resolved labels
    let blank = rpc(
        &d.socket_path,
        5,
        "v1/memory/search",
        serde_json::json!({"query": "", "limit": 10}),
    )
    .await;
    let blank_mems = blank["memories"].as_array().expect("blank memories");
    let alpha_mem = blank_mems
        .iter()
        .find(|m| m["node_id"] == node_a_id)
        .unwrap();
    assert_eq!(
        alpha_mem["relations"][0]["target_label"],
        "Beta Dispatch Engine"
    );
}

#[tokio::test]
async fn dangling_canonical_edge_preserves_target_id_without_label() {
    let d = start_daemon_at(get_temp_dir()).await;

    let res_a = rpc(
        &d.socket_path,
        1,
        "v1/memory/store",
        serde_json::json!({
            "label": "Isolated Node",
            "content": "Node with dangling relation",
            "scope": "workspace",
        }),
    )
    .await;
    let node_a_id = res_a["node_id"].as_str().unwrap().to_string();
    let dangling_target_id = uuid::Uuid::new_v4().to_string();

    {
        let conn = rusqlite::Connection::open(&d.db_path).unwrap();
        conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        conn.execute(
            "INSERT INTO edges (source, target, relation, weight, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![&node_a_id, &dangling_target_id, "communicates_via", 1.0, 1000i64],
        )
        .unwrap();
    }

    let search = rpc(
        &d.socket_path,
        2,
        "v1/memory/search",
        serde_json::json!({"query": "Isolated", "limit": 5}),
    )
    .await;
    let mems = search["memories"].as_array().unwrap();
    assert_eq!(mems.len(), 1);
    let rels = mems[0]["relations"].as_array().unwrap();
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0]["relation"], "communicates_via");
    assert_eq!(rels[0]["target_id"], dangling_target_id);
    assert!(rels[0].get("target_label").is_none());
}

#[tokio::test]
async fn canonical_edge_takes_precedence_over_property_duplicate_in_uds() {
    let d = start_daemon_at(get_temp_dir()).await;

    let res_b = rpc(
        &d.socket_path,
        1,
        "v1/memory/store",
        serde_json::json!({
            "label": "Canonical Fresh Beta",
            "content": "Fresh content",
        }),
    )
    .await;
    let node_b_id = res_b["node_id"].as_str().unwrap().to_string();

    // Store node A with property relation having stale label
    let res_a = rpc(
        &d.socket_path,
        2,
        "v1/memory/store",
        serde_json::json!({
            "label": "Alpha Client Node",
            "content": "Client node content",
            "relations": [
                {"relation": "uses", "target_id": node_b_id, "target_label": "Stale Beta Label"}
            ],
        }),
    )
    .await;
    let node_a_id = res_a["node_id"].as_str().unwrap().to_string();

    // Insert canonical edge A -> B
    {
        let conn = rusqlite::Connection::open(&d.db_path).unwrap();
        conn.execute(
            "INSERT INTO edges (source, target, relation, weight, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![&node_a_id, &node_b_id, "uses", 1.0, 1000i64],
        )
        .unwrap();
    }

    let search = rpc(
        &d.socket_path,
        3,
        "v1/memory/search",
        serde_json::json!({"query": "Alpha", "limit": 5}),
    )
    .await;
    let mems = search["memories"].as_array().unwrap();
    assert_eq!(mems.len(), 1);
    let rels = mems[0]["relations"].as_array().unwrap();
    assert_eq!(rels.len(), 1, "Duplicate relation must be deduplicated");
    assert_eq!(rels[0]["relation"], "uses");
    assert_eq!(rels[0]["target_id"], node_b_id);
    assert_eq!(rels[0]["target_label"], "Canonical Fresh Beta");
}
