use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn get_temp_dir() -> PathBuf {
    let rand_val = uuid::Uuid::new_v4().to_string();
    let path = std::env::temp_dir().join(format!("brain-pipeline-test-{}", rand_val));
    fs::create_dir_all(&path).unwrap();
    path
}

async fn send_command(socket_path: &PathBuf, action: &str, payload: &str) -> Vec<serde_json::Value> {
    let stream = UnixStream::connect(socket_path).await.expect("Failed to connect to UDS socket");
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let req = serde_json::json!({
        "action": action,
        "payload": payload
    });

    let req_str = req.to_string() + "\n";
    writer.write_all(req_str.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut lines = Vec::new();
    let mut line = String::new();
    while reader.read_line(&mut line).await.unwrap() > 0 {
        let val: serde_json::Value = serde_json::from_str(&line).unwrap();
        lines.push(val.clone());
        line.clear();

        // If it's a legacy response or the end of a stream, stop reading
        if let Some(status) = val.get("status") {
            if status == "ok" || status == "error" {
                break;
            }
        }
        if let Some(t) = val.get("type") {
            if t == "stream_end" || t == "stream_cancelled" {
                break;
            }
        }
    }
    lines
}

#[tokio::test]
async fn test_pipeline_ingest_consolidate_restart_persistence() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let rand_val = uuid::Uuid::new_v4().to_string().chars().take(8).collect::<String>();
    let socket_path = PathBuf::from(format!("/tmp/t-{}.sock", rand_val));
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    println!("Starting first daemon instance...");
    let mut child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("PYO3_PYTHON", std::env::var("PYO3_PYTHON").unwrap_or_default())
        .env("BRAIN_HEALTH_PORT", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon process");

    // Wait for UDS socket to become ready
    let mut ready = false;
    for _ in 0..50 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if !ready {
        if let Ok(Some(status)) = child.try_wait() {
            println!("Daemon exited early with status: {:?}", status);
        }
        let mut stderr = String::new();
        if let Some(mut err_pipe) = child.stderr.take() {
            use std::io::Read;
            err_pipe.read_to_string(&mut stderr).unwrap();
            println!("Daemon stderr:\n{}", stderr);
        }
        let mut stdout = String::new();
        if let Some(mut out_pipe) = child.stdout.take() {
            use std::io::Read;
            out_pipe.read_to_string(&mut stdout).unwrap();
            println!("Daemon stdout:\n{}", stdout);
        }
    }
    assert!(ready, "Daemon 1 did not bind socket in time");

    // 1. Ingest event
    println!("Ingesting test memory event...");
    let ingest_responses = send_command(&socket_path, "ingest", "Antigravity is Google Deepmind's powerful coding assistant.").await;
    assert!(!ingest_responses.is_empty(), "No response from daemon on ingest");
    assert_eq!(ingest_responses[0].get("status").and_then(|s| s.as_str()), Some("ok"), "Ingest failed");

    // 2. Wait for background consolidation (ticks every 30 seconds)
    println!("Waiting 32 seconds for background consolidation tick...");
    tokio::time::sleep(Duration::from_secs(32)).await;

    // 3. Query to verify it resides in LTM
    println!("Querying retrieved memory...");
    let query_responses = send_command(&socket_path, "query", "Antigravity").await;
    let mut query_output = String::new();
    for resp in &query_responses {
        if let Some(content) = resp.get("content") {
            query_output.push_str(content.as_str().unwrap_or(""));
        }
    }
    println!("Query Output: {}", query_output);
    assert!(query_output.to_lowercase().contains("antigravity"), "Memory node not found in LTM");

    // 4. Terminate first daemon instance
    println!("Stopping daemon instance 1...");
    let pid = child.id() as i32;
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    let _ = child.wait().unwrap();

    // Verify socket cleanup
    for _ in 0..50 {
        if !socket_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(!socket_path.exists(), "Socket not cleaned up by daemon 1");

    // 5. Restart daemon instance pointing to the same databases
    println!("Starting second daemon instance (restart persistence check)...");
    let mut child2 = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("PYO3_PYTHON", std::env::var("PYO3_PYTHON").unwrap_or_default())
        .env("BRAIN_HEALTH_PORT", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon process");

    // Wait for UDS socket to become ready
    let mut ready2 = false;
    for _ in 0..50 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready2 = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if !ready2 {
        if let Ok(Some(status)) = child2.try_wait() {
            println!("Daemon 2 exited early with status: {:?}", status);
        }
        let mut stderr = String::new();
        if let Some(mut err_pipe) = child2.stderr.take() {
            use std::io::Read;
            err_pipe.read_to_string(&mut stderr).unwrap();
            println!("Daemon 2 stderr:\n{}", stderr);
        }
        let mut stdout = String::new();
        if let Some(mut out_pipe) = child2.stdout.take() {
            use std::io::Read;
            out_pipe.read_to_string(&mut stdout).unwrap();
            println!("Daemon 2 stdout:\n{}", stdout);
        }
    }
    assert!(ready2, "Daemon 2 did not bind socket in time");

    // 6. Query again to verify memories survive process restart
    println!("Querying retrieved memory after restart...");
    let query_responses2 = send_command(&socket_path, "query", "Antigravity").await;
    let mut query_output2 = String::new();
    for resp in &query_responses2 {
        if let Some(content) = resp.get("content") {
            query_output2.push_str(content.as_str().unwrap_or(""));
        }
    }
    println!("Query Output after restart: {}", query_output2);
    assert!(query_output2.to_lowercase().contains("antigravity"), "Memory node did not survive daemon restart!");

    // 7. Stop daemon 2
    println!("Stopping daemon instance 2...");
    let pid2 = child2.id() as i32;
    unsafe {
        libc::kill(pid2, libc::SIGTERM);
    }
    let _ = child2.wait().unwrap();

    fs::remove_dir_all(&test_dir).unwrap();
    println!("Pipeline restart persistence test passed successfully!");
}
