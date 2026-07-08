use async_trait::async_trait;
use brain_core::errors::BrainError;
use brain_core::events::StreamEvent;
use brain_domain::{SessionId, Message};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

/// Custom options configuring query executions.
#[derive(Debug, Clone, Default)]
pub struct ExecutionOptions {
    /// Optional model identifier.
    pub model: Option<String>,
    /// Set to true to execute in deep planning goal mode.
    pub run_goal_mode: bool,
    /// Extensible custom runtime parameter key-values.
    pub custom_parameters: std::collections::HashMap<String, String>,
}

/// Structured parameter payload initiating an execution.
pub struct ExecutionRequest {
    /// Unique identifier of the session.
    pub session_id: SessionId,
    /// The user input text prompt.
    pub prompt: String,
    /// Execution options.
    pub options: ExecutionOptions,
    /// Token for hierarchical cancellation.
    pub cancellation_token: CancellationToken,
}

/// Opaque wrapper encapsulating streaming events and cancellation controls.
pub struct EventReceiver {
    rx: UnboundedReceiver<Result<StreamEvent, BrainError>>,
    cancellation_token: CancellationToken,
}

impl EventReceiver {
    /// Creates a new `EventReceiver` wrapping a channel receiver and token.
    pub fn new(
        rx: UnboundedReceiver<Result<StreamEvent, BrainError>>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self { rx, cancellation_token }
    }

    /// Receives the next sequential event. Returns None if stream completed.
    pub async fn recv(&mut self) -> Option<Result<StreamEvent, BrainError>> {
        self.rx.recv().await
    }

    /// Triggers hierarchical cancellation of the active generation.
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}

/// Summary overview describing a conversation thread.
pub struct SessionSummary {
    /// Unique identifier of the session.
    pub id: SessionId,
    /// User-friendly descriptive title.
    pub title: String,
    /// Time when the session thread was last updated.
    pub updated_at: std::time::SystemTime,
    /// Whether the session is pinned.
    pub pinned: bool,
    /// Whether the session is archived.
    pub archived: bool,
}

/// Abstract contract decoupling presentation viewports from execution modes.
#[async_trait]
pub trait ExecutionClient: Send + Sync {
    /// Submits a query request and returns a cancellable stream receiver.
    async fn execute(&self, req: ExecutionRequest) -> Result<EventReceiver, BrainError>;

    /// Lists all historical session summaries.
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, BrainError>;

    /// Loads historical messages for the given session.
    async fn load_session(&self, id: SessionId) -> Result<Vec<Message>, BrainError>;

    /// Permanently deletes a historical session.
    async fn delete_session(&self, id: SessionId) -> Result<(), BrainError>;

    /// Approves or denies a tool call.
    async fn approve_tool_call(&self, call_id: brain_core::events::ToolCallId, approved: bool) -> Result<(), BrainError>;

    /// Searches historical messages across all sessions.
    async fn search_messages(&self, query: &str) -> Result<Vec<Message>, BrainError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use brain_core::events::{EventMetadata, StreamEventKind};
    use uuid::Uuid;

    struct MockClient;

    #[async_trait]
    impl ExecutionClient for MockClient {
        async fn execute(&self, req: ExecutionRequest) -> Result<EventReceiver, BrainError> {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let cancellation_token = req.cancellation_token.clone();
            
            tokio::spawn(async move {
                let event = StreamEvent {
                    metadata: EventMetadata {
                        execution_id: Uuid::new_v4(),
                        sequence: 1,
                        timestamp: std::time::SystemTime::now(),
                    },
                    kind: StreamEventKind::Token("Hello".to_string()),
                };
                let _ = tx.send(Ok(event));
            });

            Ok(EventReceiver::new(rx, cancellation_token))
        }

        async fn list_sessions(&self) -> Result<Vec<SessionSummary>, BrainError> {
            Ok(vec![])
        }

        async fn load_session(&self, _id: SessionId) -> Result<Vec<Message>, BrainError> {
            Ok(vec![])
        }

        async fn delete_session(&self, _id: SessionId) -> Result<(), BrainError> {
            Ok(())
        }

        async fn approve_tool_call(&self, _call_id: brain_core::events::ToolCallId, _approved: bool) -> Result<(), BrainError> {
            Ok(())
        }

        async fn search_messages(&self, _query: &str) -> Result<Vec<Message>, BrainError> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_mock_client_streaming() {
        let client = MockClient;
        let token = CancellationToken::new();
        let req = ExecutionRequest {
            session_id: SessionId::new(),
            prompt: "Hi".to_string(),
            options: ExecutionOptions::default(),
            cancellation_token: token,
        };
        let mut receiver = client.execute(req).await.unwrap();
        let first = receiver.recv().await.unwrap().unwrap();
        if let StreamEventKind::Token(val) = first.kind {
            assert_eq!(val, "Hello");
        } else {
            panic!("Expected Token");
        }
    }
}
