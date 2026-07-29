use brain_core::events::StreamEventKind;
use brain_tui::client::{ExecutionClient, ExecutionOptions, ExecutionRequest, UdsClient};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_uds_client_execute() {
    let client = UdsClient::default();
    let token = CancellationToken::new();
    let req = ExecutionRequest {
        session_id: brain_domain::SessionId::new(),
        prompt: "test query".to_string(),
        options: ExecutionOptions::default(),
        cancellation_token: token,
        workspace_context: None,
    };

    match client.execute(req).await {
        Ok(mut rx) => {
            println!("Successfully connected to UDS daemon and executed query.");
            let mut got_events = false;
            while let Some(event) = rx.recv().await {
                match event {
                    Ok(ev) => {
                        got_events = true;
                        println!("Received event kind: {:?}", ev.kind);
                        if let StreamEventKind::Finished { .. } = ev.kind {
                            break;
                        }
                    }
                    Err(e) => {
                        panic!("Stream returned error: {:?}", e);
                    }
                }
            }
            assert!(got_events, "Should have received at least one stream event");
        }
        Err(e) => {
            println!(
                "Skipping UDS integration test because daemon is unreachable: {:?}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_daemon_returns_error_for_unknown_action() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let path = "/Users/ritikpathania/.brain/daemon.sock";
    if !std::path::Path::new(path).exists() {
        println!("Skipping: daemon socket not present");
        return;
    }
    let mut stream = match UnixStream::connect(path).await {
        Ok(s) => s,
        Err(_) => {
            println!("Skipping: daemon unreachable");
            return;
        }
    };

    stream
        .write_all(b"{\"action\":\"nonexistent_xyz\",\"payload\":\"test\"}\n")
        .await
        .unwrap();
    stream.flush().await.unwrap();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();

    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["status"], "error");
    let err = resp["message"].as_str().unwrap_or("");
    assert!(
        err.to_lowercase().contains("unknown action"),
        "Expected 'unknown action' in error, got: {}",
        err
    );
}
