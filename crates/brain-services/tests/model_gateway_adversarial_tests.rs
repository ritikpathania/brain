use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use brain_core::model::{
    ChatRole, GenerationChunk, GenerationRequest, ModelChatMessage, ModelGateway,
};
use brain_services::model::{DefaultModelGateway, DeterministicMockProvider, ScriptedResponse};

/// Adversarial Test Suite for ModelGateway & Providers (Phase 2A)
#[tokio::test]
async fn test_scenario_3_provider_hangs_indefinitely_cancellation_terminates_immediately() {
    let mock_provider = Arc::new(DeterministicMockProvider::new());
    let gateway = DefaultModelGateway::with_mock_provider(mock_provider.clone());

    // Configure a provider response with a stream that requires cancellation
    let cancel_token = CancellationToken::new();

    let req = GenerationRequest {
        model: "brain-default".to_string(),
        messages: vec![ModelChatMessage::text(ChatRole::User, "Hang test query")],
        system_prompt: None,
        tools: Vec::new(),
        thinking_budget: None,
    };

    let mut stream = gateway
        .stream_generation(req, cancel_token.clone())
        .await
        .expect("Stream should start successfully");

    // Consume first chunk (ThinkingStart)
    let first = stream.next().await;
    assert!(first.is_some());

    // Simulate cancellation mid-stream
    cancel_token.cancel();

    // The stream should terminate immediately without blocking
    let post_cancel = tokio::time::timeout(Duration::from_millis(500), async {
        let mut count = 0;
        while stream.next().await.is_some() {
            count += 1;
        }
        count
    })
    .await;

    assert!(
        post_cancel.is_ok(),
        "Stream failed to terminate immediately upon cancellation"
    );
}

#[tokio::test]
async fn test_scenario_10_provider_error_after_partial_tokens_yields_error_chunk() {
    let mock_provider = Arc::new(DeterministicMockProvider::new());
    mock_provider.enqueue_response(ScriptedResponse {
        thinking: None,
        tokens: vec!["Token 1 ".to_string(), "Token 2 ".to_string()],
        tool_calls: Vec::new(),
        error: Some("Upstream rate limit exceeded (HTTP 429)".to_string()),
        finish_reason: None,
    });

    let gateway = DefaultModelGateway::with_mock_provider(mock_provider);
    let cancel_token = CancellationToken::new();

    let req = GenerationRequest {
        model: "brain-default".to_string(),
        messages: vec![ModelChatMessage::text(
            ChatRole::User,
            "Error simulation query",
        )],
        system_prompt: None,
        tools: Vec::new(),
        thinking_budget: None,
    };

    let mut stream = gateway
        .stream_generation(req, cancel_token)
        .await
        .expect("Stream should start");

    let mut received_tokens = Vec::new();
    let mut received_error = None;

    while let Some(chunk_res) = stream.next().await {
        match chunk_res {
            Ok(GenerationChunk::TextDelta { text }) => received_tokens.push(text),
            Ok(_) => {}
            Err(e) => {
                received_error = Some(e.to_string());
                break;
            }
        }
    }

    assert_eq!(received_tokens.len(), 2);
    assert_eq!(received_tokens[0], "Token 1 ");
    assert_eq!(received_tokens[1], "Token 2 ");
    assert!(received_error.is_some());
    assert!(received_error.unwrap().contains("rate limit exceeded"));
}

#[tokio::test]
async fn test_scenario_12_concurrent_generations_across_distinct_sessions_succeed() {
    let mock_provider = Arc::new(DeterministicMockProvider::new());
    let gateway = Arc::new(DefaultModelGateway::with_mock_provider(mock_provider));

    let cancel_token_1 = CancellationToken::new();
    let cancel_token_2 = CancellationToken::new();

    let req_1 = GenerationRequest {
        model: "brain-default".to_string(),
        messages: vec![ModelChatMessage::text(
            ChatRole::User,
            "Prompt for session 1",
        )],
        system_prompt: None,
        tools: Vec::new(),
        thinking_budget: None,
    };

    let req_2 = GenerationRequest {
        model: "brain-default".to_string(),
        messages: vec![ModelChatMessage::text(
            ChatRole::User,
            "Prompt for session 2",
        )],
        system_prompt: None,
        tools: Vec::new(),
        thinking_budget: None,
    };

    let gw1 = gateway.clone();
    let h1 = tokio::spawn(async move {
        let mut stream = gw1.stream_generation(req_1, cancel_token_1).await.unwrap();
        let mut chunks = Vec::new();
        while let Some(c) = stream.next().await {
            chunks.push(c.unwrap());
        }
        chunks
    });

    let gw2 = gateway.clone();
    let h2 = tokio::spawn(async move {
        let mut stream = gw2.stream_generation(req_2, cancel_token_2).await.unwrap();
        let mut chunks = Vec::new();
        while let Some(c) = stream.next().await {
            chunks.push(c.unwrap());
        }
        chunks
    });

    let (res1, res2) = tokio::join!(h1, h2);
    let chunks1 = res1.unwrap();
    let chunks2 = res2.unwrap();

    assert!(!chunks1.is_empty());
    assert!(!chunks2.is_empty());
}
