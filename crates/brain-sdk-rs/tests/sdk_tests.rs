use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::oneshot;

use brain_integrations::IngestionEvent;
use brain_sdk_rs::{BrainClient, ClientConfig};

fn get_temp_socket_path() -> PathBuf {
    let rand_val = rand::random::<u32>();
    std::env::temp_dir().join(format!("brain-test-sdk-{}.sock", rand_val))
}

#[tokio::test]
async fn test_sdk_successful_ingestion() {
    let socket_path = get_temp_socket_path();
    let socket_path_server = socket_path.clone();

    // Clean up any stale socket file
    let _ = std::fs::remove_file(&socket_path);

    // 1. Start a mock UDS daemon server
    let listener = UnixListener::bind(&socket_path_server).expect("Failed to bind mock server");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let server_handle = tokio::spawn(async move {
        tokio::select! {
            res = listener.accept() => {
                if let Ok((stream, _)) = res {
                    let (read, mut write) = stream.into_split();
                    let mut reader = BufReader::new(read);
                    let mut line = String::new();
                    
                    // 1. Read handshake request
                    if let Ok(n) = reader.read_line(&mut line).await {
                        if n > 0 {
                            let reply_hs = serde_json::json!({
                                "status": "success",
                                "body": "handshake ok"
                            });
                            let mut reply_hs_str = serde_json::to_string(&reply_hs).unwrap();
                            reply_hs_str.push('\n');
                            let _ = write.write_all(reply_hs_str.as_bytes()).await;
                            let _ = write.flush().await;
                        }
                    }

                    // 2. Read incoming ingestion request
                    line.clear();
                    if let Ok(n) = reader.read_line(&mut line).await {
                        if n > 0 {
                            if let Ok(request_json) = serde_json::from_str::<serde_json::Value>(&line) {
                                // Extract envelope payload
                                if let Some(payload_str) = request_json.get("payload").and_then(|p| p.as_str()) {
                                    if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(payload_str) {
                                        let event_id = envelope.get("identity")
                                            .and_then(|i| i.get("event_id"))
                                            .and_then(|id| id.as_str())
                                            .unwrap_or("");

                                        // Reply with a success ACK matching the event_id
                                        let ack_body = serde_json::json!({
                                            "sequence": 42,
                                            "event_id": event_id
                                        });
                                        let ack_body_str = serde_json::to_string(&ack_body).unwrap();
                                        let reply = serde_json::json!({
                                            "message": ack_body_str
                                        });
                                        let mut reply_str = serde_json::to_string(&reply).unwrap();
                                        reply_str.push('\n');
                                        
                                        let _ = write.write_all(reply_str.as_bytes()).await;
                                        let _ = write.flush().await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ = shutdown_rx => {}
        }
    });

    // 2. Connect client and send a message event
    let mut config = ClientConfig::default_for_socket(socket_path.clone());
    config.flush_interval = Duration::from_millis(5); // Flush immediately for testing
    
    let client = BrainClient::connect(config).await.expect("Failed to connect client");

    let event = IngestionEvent::Message {
        role: "user".to_string(),
        content: "Testing SDK client ingestion".to_string(),
        metadata: std::collections::BTreeMap::new(),
    };

    let ack = client.send(event).await.expect("Failed to send event");

    // 3. Assertions
    assert_eq!(ack.sequence, 42);

    // 4. Graceful Shutdown
    client.shutdown().await;
    let _ = shutdown_tx.send(());
    let _ = server_handle.await;
    let _ = std::fs::remove_file(&socket_path);
}
