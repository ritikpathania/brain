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
#[derive(Debug, Clone, PartialEq, Eq)]
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

use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use brain_core::events::{EventMetadata, StreamEventKind, StreamEvent as CoreStreamEvent};
use uuid::Uuid;

#[derive(serde::Deserialize)]
#[serde(tag = "type")]
enum UdsStreamEvent {
    #[serde(rename = "stream_start")]
    Start {
        #[serde(rename = "streamId")]
        _stream_id: String,
        #[serde(default)]
        _metadata: serde_json::Value,
    },
    #[serde(rename = "stream_progress")]
    Progress {
        #[serde(rename = "streamId")]
        _stream_id: String,
        sequence: u64,
        progress: f64,
        message: String,
        #[serde(default)]
        _metadata: serde_json::Value,
    },
    #[serde(rename = "stream_chunk")]
    Chunk {
        #[serde(rename = "streamId")]
        _stream_id: String,
        sequence: u64,
        content: String,
        #[serde(default)]
        _metadata: serde_json::Value,
    },
    #[serde(rename = "stream_end")]
    End {
        #[serde(rename = "streamId")]
        _stream_id: String,
        sequence: u64,
        #[serde(default)]
        _metadata: serde_json::Value,
    },
    #[serde(rename = "stream_cancelled")]
    Cancelled {
        #[serde(rename = "streamId")]
        _stream_id: String,
        sequence: u64,
        #[serde(default)]
        _metadata: serde_json::Value,
    },
}

#[derive(serde::Deserialize)]
struct UdsErrorResponse {
    status: String,
    message: Option<String>,
    body: Option<String>,
}

fn map_uds_event(uds_ev: UdsStreamEvent) -> CoreStreamEvent {
    let execution_id = Uuid::new_v4();
    let timestamp = std::time::SystemTime::now();

    match uds_ev {
        UdsStreamEvent::Start { .. } => CoreStreamEvent {
            metadata: EventMetadata {
                execution_id,
                sequence: 0,
                timestamp,
            },
            kind: StreamEventKind::Stage {
                name: "Start".to_string(),
                active: true,
            },
        },
        UdsStreamEvent::Progress { sequence, progress, message, .. } => CoreStreamEvent {
            metadata: EventMetadata {
                execution_id,
                sequence,
                timestamp,
            },
            kind: StreamEventKind::Progress {
                message,
                percentage: Some(progress as f32),
            },
        },
        UdsStreamEvent::Chunk { sequence, content, .. } => CoreStreamEvent {
            metadata: EventMetadata {
                execution_id,
                sequence,
                timestamp,
            },
            kind: StreamEventKind::Token(content),
        },
        UdsStreamEvent::End { sequence, .. } => CoreStreamEvent {
            metadata: EventMetadata {
                execution_id,
                sequence,
                timestamp,
            },
            kind: StreamEventKind::Finished {
                response: "".to_string(),
            },
        },
        UdsStreamEvent::Cancelled { sequence, .. } => CoreStreamEvent {
            metadata: EventMetadata {
                execution_id,
                sequence,
                timestamp,
            },
            kind: StreamEventKind::Cancelled,
        },
    }
}

/// An execution client that communicates with the daemon over a Unix Domain Socket.
pub struct UdsClient {
    socket_path: std::path::PathBuf,
}

impl UdsClient {
    /// Creates a new `UdsClient` connecting to the specified socket path.
    pub fn new(socket_path: std::path::PathBuf) -> Self {
        Self { socket_path }
    }
}

impl Default for UdsClient {
    fn default() -> Self {
        let socket_path = if let Ok(path) = std::env::var("BRAIN_SOCKET_PATH") {
            std::path::PathBuf::from(path)
        } else if let Ok(home) = std::env::var("HOME") {
            std::path::PathBuf::from(home).join(".brain").join("daemon.sock")
        } else {
            std::path::PathBuf::from("/tmp/brain-daemon.sock")
        };
        Self::new(socket_path)
    }
}

#[async_trait]
impl ExecutionClient for UdsClient {
    async fn execute(&self, req: ExecutionRequest) -> Result<EventReceiver, BrainError> {
        let mut stream = UnixStream::connect(&self.socket_path).await.map_err(|e| {
            BrainError::Network {
                message: format!("Failed to connect to UDS daemon: {}", e),
                url: None,
            }
        })?;

        let payload = serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 1,
            "action": "query",
            "body": req.prompt
        });
        
        let mut payload_str = serde_json::to_string(&payload).unwrap();
        payload_str.push('\n');
        
        stream.write_all(payload_str.as_bytes()).await.map_err(|e| {
            BrainError::Storage {
                message: format!("Failed to send query over UDS stream: {}", e),
                source: None,
            }
        })?;
        stream.flush().await.map_err(|e| {
            BrainError::Storage {
                message: format!("Failed to flush UDS stream: {}", e),
                source: None,
            }
        })?;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let cancellation_token = req.cancellation_token.clone();

        tokio::spawn(async move {
            let (reader, _) = stream.split();
            let mut buf_reader = BufReader::new(reader);
            let mut line = String::new();

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        break;
                    }
                    res = buf_reader.read_line(&mut line) => {
                        match res {
                            Ok(0) => break,
                            Ok(_) => {
                                let trim_line = line.trim();
                                if !trim_line.is_empty() {
                                    if let Ok(uds_ev) = serde_json::from_str::<UdsStreamEvent>(trim_line) {
                                        let core_ev = map_uds_event(uds_ev);
                                        let is_finished = matches!(core_ev.kind, StreamEventKind::Finished { .. }) || matches!(core_ev.kind, StreamEventKind::Cancelled);
                                        let _ = tx.send(Ok(core_ev));
                                        if is_finished {
                                            break;
                                        }
                                    } else if let Ok(err_resp) = serde_json::from_str::<UdsErrorResponse>(trim_line) {
                                        if err_resp.status == "error" {
                                            let msg = err_resp.body.or(err_resp.message).unwrap_or_else(|| "Unknown daemon error".to_string());
                                            let _ = tx.send(Err(BrainError::Internal { message: msg }));
                                            break;
                                        }
                                    }
                                }
                                line.clear();
                            }
                            Err(e) => {
                                let _ = tx.send(Err(BrainError::Network {
                                    message: format!("Read error on UDS stream: {}", e),
                                    url: None,
                                }));
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(EventReceiver::new(rx, req.cancellation_token))
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, BrainError> {
        // Return a single default user session for single-user environment
        let session = SessionSummary {
            id: SessionId::new(),
            title: "New Conversation".to_string(),
            updated_at: std::time::SystemTime::now(),
            pinned: false,
            archived: false,
        };
        Ok(vec![session])
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
