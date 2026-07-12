use brain_tui::client::{UdsClient, ExecutionClient, ExecutionRequest, ExecutionOptions};
use tokio_util::sync::CancellationToken;
use brain_core::events::StreamEventKind;

#[tokio::test]
async fn test_uds_client_execute() {
    let client = UdsClient::default();
    let token = CancellationToken::new();
    let req = ExecutionRequest {
        session_id: brain_domain::SessionId::new(),
        prompt: "test query".to_string(),
        options: ExecutionOptions::default(),
        cancellation_token: token,
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
            println!("Skipping UDS integration test because daemon is unreachable: {:?}", e);
        }
    }
}
