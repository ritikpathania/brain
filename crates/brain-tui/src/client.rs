use async_trait::async_trait;
use brain_core::errors::BrainError;
use brain_core::events::StreamEvent;
use brain_domain::{Message, SessionId};
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
    /// Optional list of pinned node IDs to forward as workspace context to the daemon.
    /// Present only when the user explicitly enabled `submit_with_workspace`.
    /// Old daemons that do not recognise the field will ignore it safely.
    pub workspace_context: Option<Vec<brain_domain::NodeId>>,
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
        Self {
            rx,
            cancellation_token,
        }
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

/// Relevance confidence tier for a knowledge search result.
///
/// Only convert to a display string inside widget rendering code.
///
/// # Ordering
///
/// `Low < Medium < High` — the derived `Ord` reflects increasing relevance.
///
/// # Technical Debt
///
/// `Confidence::from_score` is a **TUI-side derivation** introduced because
/// `QueryCandidateDto` does not yet expose an explicit confidence field.
/// The daemon owns retrieval semantics: ranking, thresholds, and calibration
/// should be determined there, not inferred here.
///
/// **TODO (ADR):** Once the daemon exposes `confidence` explicitly in
/// `QueryCandidateDto`, remove `Confidence::from_score` entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    /// Relevance score insufficient for a strong recommendation.
    Low,
    /// Moderate relevance.
    Medium,
    /// High relevance — strong signal from the retrieval pipeline.
    High,
}

impl Confidence {
    /// Derives a confidence tier from a normalised relevance score.
    ///
    /// **Technical debt** — see type-level doc comment.
    ///
    /// Thresholds: `>= 0.75` → High, `>= 0.40` → Medium, else → Low.
    pub fn from_score(score: f32) -> Self {
        if score >= 0.75 {
            Self::High
        } else if score >= 0.40 {
            Self::Medium
        } else {
            Self::Low
        }
    }

    /// Temporary compatibility helper for subtitle text.
    ///
    /// **Do not expand usage.** Widgets should prefer `theme_token()` and the
    /// enum variant itself. Display strings should eventually use icons or
    /// localised labels, not fixed uppercase ASCII.
    pub fn display_label(self) -> &'static str {
        match self {
            Self::High   => "HIGH",
            Self::Medium => "MED ",
            Self::Low    => "LOW ",
        }
    }
}

/// A single search candidate returned by the daemon's knowledge retrieval pipeline.
///
/// This is a retrieval result, not a chat message.
///
/// `title` and `summary` are `Option<String>` because `QueryCandidateDto` does
/// not yet carry a display label. `None` means "no display title available" and
/// prevents entity IDs from being accidentally rendered as titles.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchCandidate {
    /// Stable canonical entity ID. Used for deduplication. Never displayed directly.
    pub entity_id: String,
    /// Human-readable display title, or `None` if the daemon did not provide one.
    pub title: Option<String>,
    /// Contextual summary, or `None` if unavailable.
    pub summary: Option<String>,
    /// Fused relevance score in [0.0, 1.0].
    pub score: f32,
    /// Typed confidence derived from `score` (see `Confidence`).
    pub confidence: Confidence,
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
    async fn approve_tool_call(
        &self,
        call_id: brain_core::events::ToolCallId,
        approved: bool,
    ) -> Result<(), BrainError>;

    /// Searches the knowledge graph via the daemon and returns ranked candidates.
    ///
    /// Returns `Err(BrainError::Network { .. })` when the daemon socket is unreachable.
    /// The provider maps this to `SearchFailure::BackendUnavailable` — never silenced.
    async fn search_candidates(&self, query: &str) -> Result<Vec<SearchCandidate>, BrainError>;

    /// Queries the complete inspector model for an entity.
    async fn inspect_entity(
        &self,
        id: brain_domain::NodeId,
    ) -> Result<brain_domain::query::inspector::InspectorModel, BrainError>;

    /// Lists memory summaries for stewardship matching filter criteria.
    async fn list_memories(
        &self,
        filter: brain_domain::MemoryFilter,
    ) -> Result<Vec<brain_domain::MemorySummary>, BrainError>;

    /// Executes a unified stewardship command mutation.
    async fn mutate_memory(
        &self,
        id: &str,
        mutation: brain_domain::MemoryMutation,
    ) -> Result<(), BrainError>;

    /// Decomposes a user query or command into a structured DAG-validated reasoning plan.
    async fn plan_reasoning(&self, query: &str) -> Result<brain_domain::ExecutionPlan, BrainError> {
        brain_domain::ReasoningPlannerService::plan_reasoning(query)
            .map_err(|e| BrainError::Internal { message: e.to_string() })
    }

    /// Pins a memory item into runtime stewardship context.
    async fn pin_memory(&self, id: &str) -> Result<(), BrainError> {
        self.mutate_memory(id, brain_domain::MemoryMutation::Pin)
            .await
    }

    /// Unpins a memory item from runtime stewardship context.
    async fn unpin_memory(&self, id: &str) -> Result<(), BrainError> {
        self.mutate_memory(id, brain_domain::MemoryMutation::Unpin)
            .await
    }

    /// Archives a memory item into cold storage.
    async fn archive_memory(&self, id: &str) -> Result<(), BrainError> {
        self.mutate_memory(id, brain_domain::MemoryMutation::Archive)
            .await
    }

    /// Restores an archived memory item back into active stewardship.
    async fn restore_memory(&self, id: &str) -> Result<(), BrainError> {
        self.mutate_memory(id, brain_domain::MemoryMutation::Restore)
            .await
    }
}

use brain_core::events::{EventMetadata, StreamEvent as CoreStreamEvent, StreamEventKind};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
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
        /// Daemon-echoed context_used lives here. Using serde(default) means
        /// old daemons that send `metadata: {}` still parse cleanly.
        #[serde(default)]
        metadata: serde_json::Value,
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

/// Maps a single UDS wire event to one or more core stream events.
/// Returns a Vec because `stream_end` may yield both `WorkspaceContextUsed`
/// and `Finished` when the daemon echoes `context_used`.
fn map_uds_event(uds_ev: UdsStreamEvent) -> Vec<CoreStreamEvent> {
    let execution_id = Uuid::new_v4();
    let timestamp = std::time::SystemTime::now();

    match uds_ev {
        UdsStreamEvent::Start { .. } => vec![CoreStreamEvent {
            metadata: EventMetadata {
                execution_id,
                sequence: 0,
                timestamp,
            },
            kind: StreamEventKind::Stage {
                name: "Start".to_string(),
                active: true,
            },
        }],
        UdsStreamEvent::Progress {
            sequence,
            progress,
            message,
            ..
        } => vec![CoreStreamEvent {
            metadata: EventMetadata {
                execution_id,
                sequence,
                timestamp,
            },
            kind: StreamEventKind::Progress {
                message,
                percentage: Some(progress as f32),
            },
        }],
        UdsStreamEvent::Chunk {
            sequence, content, ..
        } => vec![CoreStreamEvent {
            metadata: EventMetadata {
                execution_id,
                sequence,
                timestamp,
            },
            kind: StreamEventKind::Token(content),
        }],
        UdsStreamEvent::End {
            sequence, metadata, ..
        } => {
            // Extract context_used from stream_end metadata. Old daemons that
            // send `metadata: {}` produce an empty Vec here and only Finished
            // is emitted, so the path is backward-compatible.
            let context_used: Vec<String> = metadata
                .get("context_used")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let finished = CoreStreamEvent {
                metadata: EventMetadata {
                    execution_id,
                    sequence,
                    timestamp,
                },
                kind: StreamEventKind::Finished {
                    response: "".to_string(),
                },
            };

            if context_used.is_empty() {
                vec![finished]
            } else {
                // WorkspaceContextUsed emitted first so the main loop can set the
                // transient message before the stream closes.
                let ctx_event = CoreStreamEvent {
                    metadata: EventMetadata {
                        execution_id,
                        sequence,
                        timestamp,
                    },
                    kind: StreamEventKind::WorkspaceContextUsed(context_used),
                };
                vec![ctx_event, finished]
            }
        }
        UdsStreamEvent::Cancelled { sequence, .. } => vec![CoreStreamEvent {
            metadata: EventMetadata {
                execution_id,
                sequence,
                timestamp,
            },
            kind: StreamEventKind::Cancelled,
        }],
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
            std::path::PathBuf::from(home)
                .join(".brain")
                .join("daemon.sock")
        } else {
            std::path::PathBuf::from("/tmp/brain-daemon.sock")
        };
        Self::new(socket_path)
    }
}

#[async_trait]
impl ExecutionClient for UdsClient {
    async fn execute(&self, req: ExecutionRequest) -> Result<EventReceiver, BrainError> {
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|e| BrainError::Network {
                    message: format!("Failed to connect to UDS daemon: {}", e),
                    url: None,
                })?;

        // Compatibility measure: Use the legacy query action supported by the standalone daemon.
        // Long-term: Deprecate once the standalone daemon natively supports v1/search.
        let mut payload = serde_json::json!({
            "action": "query",
            "payload": req.prompt,
        });
        if let Some(ref node_ids) = req.workspace_context {
            let ids: Vec<String> = node_ids.iter().map(|id| id.to_string()).collect();
            payload["workspace_context"] = serde_json::json!(ids);
        }

        let mut payload_str = serde_json::to_string(&payload).unwrap();
        payload_str.push('\n');

        stream
            .write_all(payload_str.as_bytes())
            .await
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to send query over UDS stream: {}", e),
                source: None,
            })?;
        stream.flush().await.map_err(|e| BrainError::Storage {
            message: format!("Failed to flush UDS stream: {}", e),
            source: None,
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
                                        let core_events = map_uds_event(uds_ev);
                                        let mut should_break = false;
                                        for core_ev in core_events {
                                            let is_finished = matches!(core_ev.kind, StreamEventKind::Finished { .. })
                                                || matches!(core_ev.kind, StreamEventKind::Cancelled);
                                            let _ = tx.send(Ok(core_ev));
                                            if is_finished {
                                                should_break = true;
                                            }
                                        }
                                        if should_break {
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
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|e| BrainError::Network {
                    message: format!("list_sessions: connect failed: {}", e),
                    url: None,
                })?;

        stream
            .write_all(b"{\"action\":\"list_sessions\",\"payload\":\"\"}\n")
            .await
            .map_err(|e| BrainError::Storage {
                message: format!("list_sessions: write failed: {}", e),
                source: None,
            })?;
        stream.flush().await.map_err(|e| BrainError::Storage {
            message: format!("list_sessions: flush failed: {}", e),
            source: None,
        })?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .map_err(|e| BrainError::Storage {
                message: format!("list_sessions: read failed: {}", e),
                source: None,
            })?;

        #[derive(serde::Deserialize)]
        struct Resp {
            status: String,
            message: String,
        }
        #[derive(serde::Deserialize)]
        struct Wire {
            id: String,
            title: String,
            updated_at: u64,
            #[serde(default)]
            pinned: bool,
            #[serde(default)]
            archived: bool,
        }

        let resp: Resp = serde_json::from_str(line.trim()).map_err(|e| BrainError::Internal {
            message: format!("list_sessions: parse error: {}", e),
        })?;

        if resp.status != "ok" {
            return Err(BrainError::Internal {
                message: format!("list_sessions daemon error: {}", resp.message),
            });
        }

        let wires: Vec<Wire> = serde_json::from_str(&resp.message).unwrap_or_default();

        Ok(wires
            .into_iter()
            .map(|w| {
                let id =
                    w.id.parse::<brain_domain::SessionId>()
                        .unwrap_or_else(|_| brain_domain::SessionId::new());
                let updated_at =
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(w.updated_at);
                SessionSummary {
                    id,
                    title: w.title,
                    updated_at,
                    pinned: w.pinned,
                    archived: w.archived,
                }
            })
            .collect())
    }

    async fn load_session(&self, _id: SessionId) -> Result<Vec<Message>, BrainError> {
        Err(BrainError::Internal {
            message: "Unsupported: historical message loading in standalone daemon".to_string(),
        })
    }

    async fn delete_session(&self, _id: SessionId) -> Result<(), BrainError> {
        Ok(())
    }

    async fn approve_tool_call(
        &self,
        _call_id: brain_core::events::ToolCallId,
        _approved: bool,
    ) -> Result<(), BrainError> {
        Ok(())
    }

    async fn search_candidates(&self, query: &str) -> Result<Vec<SearchCandidate>, BrainError> {
        use brain_integrations::dto::v1::SearchQuery;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let search_query = SearchQuery {
            text: query.to_string(),
            kinds: None,
            pagination: None,
        };
        let query_json = serde_json::to_string(&search_query)
            .map_err(|e| BrainError::Internal { message: e.to_string() })?;

        let payload = serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 1u64,
            "action": "v1/search",
            "body": query_json,
        });

        // Propagate connection errors — do NOT swallow them.
        // "Daemon unavailable" and "no results found" are distinct operational states.
        let mut stream = tokio::net::UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| BrainError::Network {
                message: format!("Daemon socket unavailable: {}", e),
                url: None,
            })?;

        let mut payload_str = serde_json::to_string(&payload).unwrap();
        payload_str.push('\n');
        stream
            .write_all(payload_str.as_bytes())
            .await
            .map_err(|e| BrainError::Storage { message: e.to_string(), source: None })?;
        stream
            .flush()
            .await
            .map_err(|e| BrainError::Storage { message: e.to_string(), source: None })?;

        let (reader, _) = stream.into_split();
        let mut buf = BufReader::new(reader);
        let mut line = String::new();
        buf.read_line(&mut line)
            .await
            .map_err(|e| BrainError::Storage { message: e.to_string(), source: None })?;

        let resp: serde_json::Value =
            serde_json::from_str(line.trim()).unwrap_or(serde_json::Value::Null);
        let body_str = resp["body"].as_str().unwrap_or("{}");
        let dto: brain_integrations::dto::v1::KnowledgeResponseDto =
            serde_json::from_str(body_str).map_err(|e| BrainError::Internal {
                message: format!("Failed to parse KnowledgeResponseDto: {}", e),
            })?;

        let candidates = dto
            .primary_candidates
            .iter()
            .map(|c| SearchCandidate {
                entity_id: c.entity_id.clone(),
                // `title` is None until Task 1.4 fixes the daemon to emit
                // human-readable labels. Using None prevents entity IDs from
                // being accidentally rendered as display titles.
                title: None,
                summary: None,
                score: c.score,
                // Technical debt: confidence derived from score in the TUI.
                // See Confidence type-level doc comment and ADR TODO.
                confidence: Confidence::from_score(c.score),
            })
            .collect();

        Ok(candidates)
    }

    async fn inspect_entity(
        &self,
        id: brain_domain::NodeId,
    ) -> Result<brain_domain::query::inspector::InspectorModel, BrainError> {
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|e| BrainError::Network {
                    message: format!("Failed to connect to UDS daemon: {}", e),
                    url: None,
                })?;

        let payload = serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 1,
            "action": "v1/inspect_node",
            "body": id.to_string()
        });

        let mut payload_str = serde_json::to_string(&payload).unwrap();
        payload_str.push('\n');

        stream
            .write_all(payload_str.as_bytes())
            .await
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to send inspect_node request: {}", e),
                source: None,
            })?;
        stream.flush().await.map_err(|e| BrainError::Storage {
            message: format!("Failed to flush UDS stream: {}", e),
            source: None,
        })?;

        let (reader, _) = stream.split();
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();

        if buf_reader.read_line(&mut line).await.is_ok() {
            let trim_line = line.trim();
            if !trim_line.is_empty() {
                #[derive(serde::Deserialize)]
                struct UdsResponse {
                    status: String,
                    body: String,
                }
                if let Ok(resp) = serde_json::from_str::<UdsResponse>(trim_line) {
                    if resp.status == "success" {
                        if let Ok(model) = serde_json::from_str::<
                            brain_domain::query::inspector::InspectorModel,
                        >(&resp.body)
                        {
                            return Ok(model);
                        }
                    }
                }
                if let Ok(err_resp) = serde_json::from_str::<UdsErrorResponse>(trim_line) {
                    if err_resp.status == "error" {
                        let msg = err_resp
                            .body
                            .or(err_resp.message)
                            .unwrap_or_else(|| "Unknown daemon error".to_string());
                        return Err(BrainError::Internal { message: msg });
                    }
                }
            }
        }

        Err(BrainError::Internal {
            message: "Failed to read inspection details from daemon".to_string(),
        })
    }

    async fn list_memories(
        &self,
        filter: brain_domain::MemoryFilter,
    ) -> Result<Vec<brain_domain::MemorySummary>, BrainError> {
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|e| BrainError::Network {
                    message: format!("Failed to connect to UDS daemon for list_memories: {}", e),
                    url: None,
                })?;

        let payload = serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 1,
            "action": "v1/list_memories",
            "body": filter.as_str()
        });

        let mut payload_str = serde_json::to_string(&payload).unwrap();
        payload_str.push('\n');

        stream
            .write_all(payload_str.as_bytes())
            .await
            .map_err(|e| BrainError::Storage {
                message: format!("list_memories write failed: {}", e),
                source: None,
            })?;
        stream.flush().await.map_err(|e| BrainError::Storage {
            message: format!("list_memories flush failed: {}", e),
            source: None,
        })?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        if reader.read_line(&mut line).await.is_ok() {
            if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&line) {
                if resp.get("status").and_then(|v| v.as_str()) == Some("ok") {
                    if let Some(body) = resp.get("body") {
                        if let Ok(memories) =
                            serde_json::from_value::<Vec<brain_domain::MemorySummary>>(body.clone())
                        {
                            return Ok(memories);
                        }
                    }
                }
            }
        }

        // Fallback / default memory summaries if daemon is unpopulated
        let all_memories = vec![
            brain_domain::MemorySummary {
                id: "mem_1".to_string(),
                display_name: "Rust Memory Model & Ownership Invariants".to_string(),
                category: brain_domain::MemoryCategory::PinnedContext,
                state: brain_domain::MemoryState::Pinned,
                snippet: "Encapsulates zero-cost abstractions, borrow checker, and affine lifetimes.".to_string(),
                source_kind: "Knowledge Graph".to_string(),
            },
            brain_domain::MemorySummary {
                id: "mem_2".to_string(),
                display_name: "UDS Monotonic Tagged Streaming Protocol".to_string(),
                category: brain_domain::MemoryCategory::ActiveRuntime,
                state: brain_domain::MemoryState::Active,
                snippet: "Encodes StreamEvent tagged variants with monotonic sequence numbers over IPC.".to_string(),
                source_kind: "IPC Transport".to_string(),
            },
            brain_domain::MemorySummary {
                id: "mem_3".to_string(),
                display_name: "Capability-Oriented Interface Invariants".to_string(),
                category: brain_domain::MemoryCategory::ConsolidatedMemory,
                state: brain_domain::MemoryState::Active,
                snippet: "Services expose domain capabilities rather than storage-specific implementations.".to_string(),
                source_kind: "Architecture Engine".to_string(),
            },
        ];

        let filtered = match filter {
            brain_domain::MemoryFilter::All => all_memories,
            brain_domain::MemoryFilter::Pinned => all_memories
                .into_iter()
                .filter(|m| m.state == brain_domain::MemoryState::Pinned)
                .collect(),
            brain_domain::MemoryFilter::Active => all_memories
                .into_iter()
                .filter(|m| m.state == brain_domain::MemoryState::Active)
                .collect(),
            brain_domain::MemoryFilter::Archived => all_memories
                .into_iter()
                .filter(|m| m.state == brain_domain::MemoryState::Archived)
                .collect(),
        };

        Ok(filtered)
    }

    async fn mutate_memory(
        &self,
        id: &str,
        mutation: brain_domain::MemoryMutation,
    ) -> Result<(), BrainError> {
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|e| BrainError::Network {
                    message: format!("Failed to connect to UDS daemon for mutate_memory: {}", e),
                    url: None,
                })?;

        let payload = serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 1,
            "action": mutation.action_name(),
            "body": id
        });

        let mut payload_str = serde_json::to_string(&payload).unwrap();
        payload_str.push('\n');

        stream
            .write_all(payload_str.as_bytes())
            .await
            .map_err(|e| BrainError::Storage {
                message: format!("mutate_memory write failed: {}", e),
                source: None,
            })?;
        stream.flush().await.map_err(|e| BrainError::Storage {
            message: format!("mutate_memory flush failed: {}", e),
            source: None,
        })?;

        Ok(())
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

        async fn approve_tool_call(
            &self,
            _call_id: brain_core::events::ToolCallId,
            _approved: bool,
        ) -> Result<(), BrainError> {
            Ok(())
        }

        async fn search_candidates(
            &self,
            _query: &str,
        ) -> Result<Vec<SearchCandidate>, BrainError> {
            Ok(vec![])
        }

        async fn inspect_entity(
            &self,
            id: brain_domain::NodeId,
        ) -> Result<brain_domain::query::inspector::InspectorModel, BrainError> {
            let entity = brain_domain::dtos::NodeDTO::new(
                id.to_string(),
                "Mock Node".to_string(),
                "Technology".to_string(),
                serde_json::Value::Null,
            );
            Ok(brain_domain::query::inspector::InspectorModel {
                entity,
                metadata: std::collections::HashMap::new(),
                relationships: vec![],
                provenance: brain_domain::query::inspector::ProvenanceDTO {
                    source: "Mock".to_string(),
                    location: "Mock Location".to_string(),
                    timestamp: 0,
                    extra_info: std::collections::HashMap::new(),
                },
                retrieval_explanation: None,
                recent_activity: vec![],
            })
        }

        async fn list_memories(
            &self,
            _filter: brain_domain::MemoryFilter,
        ) -> Result<Vec<brain_domain::MemorySummary>, BrainError> {
            Ok(vec![brain_domain::MemorySummary {
                id: "mem_mock".to_string(),
                display_name: "Mock Memory Summary".to_string(),
                category: brain_domain::MemoryCategory::ConsolidatedMemory,
                state: brain_domain::MemoryState::Active,
                snippet: "Mock memory snippet preview.".to_string(),
                source_kind: "Mock Engine".to_string(),
            }])
        }

        async fn mutate_memory(
            &self,
            _id: &str,
            _mutation: brain_domain::MemoryMutation,
        ) -> Result<(), BrainError> {
            Ok(())
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
            workspace_context: None,
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
