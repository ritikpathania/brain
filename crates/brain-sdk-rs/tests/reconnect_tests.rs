use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

use brain_integrations::IngestionEvent;
use brain_sdk_rs::{BrainClient, ClientConfig};

fn get_temp_socket_path() -> PathBuf {
    let rand_val = rand::random::<u32>();
    std::env::temp_dir().join(format!("brain-test-reconnect-{}.sock", rand_val))
}

/// Helper: accept one connection, read one event, reply with an ACK, then close.
/// Returns the event_id it received.
async fn serve_one_event(listener: &UnixListener) -> String {
    let (stream, _) = listener.accept().await.expect("accept failed");
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();

    // 1. Read handshake request
    let n = reader.read_line(&mut line).await.expect("read_line failed");
    assert!(n > 0, "Expected to read a handshake line");
    
    // Reply with handshake ok
    let reply_hs = serde_json::json!({
        "status": "success",
        "body": "handshake ok"
    });
    let mut reply_hs_str = serde_json::to_string(&reply_hs).unwrap();
    reply_hs_str.push('\n');
    let _ = write.write_all(reply_hs_str.as_bytes()).await;
    let _ = write.flush().await;

    // 2. Read the ingest event request
    line.clear();
    let n = reader.read_line(&mut line).await.expect("read_line failed");
    assert!(n > 0, "Expected to read an event line");

    let request_json: serde_json::Value =
        serde_json::from_str(&line).expect("Failed to parse request JSON");

    let payload_str = request_json
        .get("payload")
        .and_then(|p| p.as_str())
        .expect("Missing payload");
    let envelope: serde_json::Value =
        serde_json::from_str(payload_str).expect("Failed to parse envelope JSON");
    let event_id = envelope
        .get("identity")
        .and_then(|i| i.get("event_id"))
        .and_then(|id| id.as_str())
        .expect("Missing event_id")
        .to_string();

    // Reply with ACK
    let ack_body = serde_json::json!({
        "sequence": 1,
        "event_id": event_id
    });
    let reply = serde_json::json!({
        "message": serde_json::to_string(&ack_body).unwrap()
    });
    let mut reply_str = serde_json::to_string(&reply).unwrap();
    reply_str.push('\n');
    let _ = write.write_all(reply_str.as_bytes()).await;
    let _ = write.flush().await;

    event_id
}

/// Test: client reconnects after the daemon (mock server) restarts.
///
/// Flow:
/// 1. Start mock server, connect client, send event #1, receive ACK.
/// 2. Drop the server listener (simulates daemon crash).
/// 3. Rebind a new listener on the same socket path (simulates daemon restart).
/// 4. Send event #2 — client must reconnect and deliver it.
#[tokio::test]
async fn test_reconnect_after_daemon_restart() {
    let socket_path = get_temp_socket_path();
    let _ = std::fs::remove_file(&socket_path);

    // --- Phase 1: Initial connection ---
    let listener1 = UnixListener::bind(&socket_path).expect("Failed to bind mock server (phase 1)");

    let mut config = ClientConfig::default_for_socket(socket_path.clone());
    config.flush_interval = Duration::from_millis(5);

    let client = BrainClient::connect(config)
        .await
        .expect("Failed to connect client");

    // Send event #1
    let send1 = tokio::spawn({
        let client_ref = unsafe {
            // SAFETY: We hold client alive for the duration of this test.
            // This is a test-only workaround since BrainClient doesn't implement Clone.
            &*(&client as *const BrainClient)
        };
        let event = IngestionEvent::Message {
            role: "user".to_string(),
            content: "Event before restart".to_string(),
            metadata: BTreeMap::new(),
        };
        async move { client_ref.send(event).await }
    });

    let _event_id_1 = serve_one_event(&listener1).await;
    let ack1 = send1.await.expect("send1 join failed").expect("send1 failed");
    assert_eq!(ack1.sequence, 1);

    // --- Phase 2: Simulate daemon crash ---
    drop(listener1);
    // Remove the socket file so we can rebind
    let _ = std::fs::remove_file(&socket_path);

    // Small delay to let the client detect the disconnect
    tokio::time::sleep(Duration::from_millis(50)).await;

    // --- Phase 3: Restart daemon ---
    let listener2 = UnixListener::bind(&socket_path).expect("Failed to bind mock server (phase 2)");

    // Send event #2 — this should trigger reconnection
    let event2 = IngestionEvent::Message {
        role: "assistant".to_string(),
        content: "Event after restart".to_string(),
        metadata: BTreeMap::new(),
    };

    // Use a timeout to avoid hanging if reconnection fails
    let send2_result = tokio::time::timeout(Duration::from_secs(5), async {
        // The serve_one_event needs to run concurrently with client.send()
        let serve_handle = tokio::spawn(async move { serve_one_event(&listener2).await });

        let ack = client.send(event2).await.expect("send2 failed");
        let _event_id_2 = serve_handle.await.expect("serve_handle join failed");
        ack
    })
    .await;

    let ack2 = send2_result.expect("Reconnection timed out after 5 seconds");
    assert_eq!(ack2.sequence, 1);

    // --- Cleanup ---
    client.shutdown().await;
    let _ = std::fs::remove_file(&socket_path);
}

/// Test: shutdown is responsive even when disconnected (no daemon running).
///
/// This guards against the original deadlock where shutdown commands were
/// blocked by the reconnect loop.
#[tokio::test]
async fn test_shutdown_while_disconnected() {
    let socket_path = get_temp_socket_path();
    // Intentionally do NOT create a listener — there's no daemon.
    let _ = std::fs::remove_file(&socket_path);

    let mut config = ClientConfig::default_for_socket(socket_path.clone());
    config.flush_interval = Duration::from_millis(100);

    let client = BrainClient::connect(config)
        .await
        .expect("Failed to connect client");

    // Wait a bit so the runtime enters the reconnect loop
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Shutdown must complete within 1 second even though no daemon is running
    let result = tokio::time::timeout(Duration::from_secs(1), client.shutdown()).await;

    assert!(
        result.is_ok(),
        "shutdown() should complete promptly even when disconnected"
    );
}
