use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::{BackpressurePolicy, ClientCommand};
use brain_domain::{AdapterId, ClientId, ConversationId, EventId, SessionId, WorkspaceId};
use brain_integrations::{EventIdentity, IngestionEnvelope, IngestionEvent, dto::v1};

#[derive(Debug, thiserror::Error, Clone)]
pub enum BrainSdkError {
    #[error("Socket connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Daemon error response: {0}")]
    DaemonError(String),
    #[error("Send error: {0}")]
    SendError(String),
    #[error("Queue is full")]
    QueueFull,
    #[error("Client is shutting down")]
    ShuttingDown,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct IngestAck {
    pub sequence: u64,
    pub event_id: EventId,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq)]
pub struct ReplayResponse {
    pub events: Vec<IngestionEnvelope>,
    pub last_sequence: u64,
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub socket_path: PathBuf,
    pub max_queued_events: usize,
    pub max_batch_size: usize,
    pub max_batch_bytes: usize,
    pub flush_interval: Duration,
    pub backpressure_policy: BackpressurePolicy,

    pub adapter_id: AdapterId,
    pub client_id: ClientId,
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub conversation_id: Option<ConversationId>,
}

impl ClientConfig {
    pub fn default_for_socket(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            max_queued_events: 1000,
            max_batch_size: 50,
            max_batch_bytes: 1024 * 1024,
            flush_interval: Duration::from_millis(50),
            backpressure_policy: BackpressurePolicy::Block,
            adapter_id: AdapterId::new("rust-sdk-adapter"),
            client_id: ClientId::new("rust-sdk-client"),
            workspace_id: WorkspaceId::new("default-workspace"),
            session_id: SessionId::new(),
            conversation_id: None,
        }
    }
}

#[async_trait::async_trait]
pub trait ReplayStrategy: Send + Sync {
    async fn record(&self, envelope: IngestionEnvelope) -> Result<(), BrainSdkError>;
    async fn acknowledge(&self, event_id: &EventId) -> Result<(), BrainSdkError>;
    async fn reconcile(
        &self,
        replay: ReplayResponse,
    ) -> Result<Vec<IngestionEnvelope>, BrainSdkError>;
    async fn get_unacknowledged(&self) -> Vec<IngestionEnvelope>;
}

pub struct InMemoryReplayStrategy {
    pending: Mutex<BTreeMap<EventId, IngestionEnvelope>>,
}

impl InMemoryReplayStrategy {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(BTreeMap::new()),
        }
    }
}

impl Default for InMemoryReplayStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ReplayStrategy for InMemoryReplayStrategy {
    async fn record(&self, envelope: IngestionEnvelope) -> Result<(), BrainSdkError> {
        let mut guard = self.pending.lock().await;
        guard.insert(envelope.identity.event_id, envelope);
        Ok(())
    }

    async fn acknowledge(&self, event_id: &EventId) -> Result<(), BrainSdkError> {
        let mut guard = self.pending.lock().await;
        guard.remove(event_id);
        Ok(())
    }

    async fn reconcile(
        &self,
        replay: ReplayResponse,
    ) -> Result<Vec<IngestionEnvelope>, BrainSdkError> {
        let mut guard = self.pending.lock().await;
        for ev in &replay.events {
            guard.remove(&ev.identity.event_id);
        }
        Ok(guard.values().cloned().collect())
    }

    async fn get_unacknowledged(&self) -> Vec<IngestionEnvelope> {
        let guard = self.pending.lock().await;
        guard.values().cloned().collect()
    }
}

pub trait BatchStrategy: Send + Sync {
    fn push(&mut self, envelope: IngestionEnvelope);
    fn should_flush(&self) -> bool;
    fn drain(&mut self) -> Vec<IngestionEnvelope>;
}

pub struct DefaultBatchStrategy {
    buffer: Vec<IngestionEnvelope>,
    max_batch_size: usize,
    max_batch_bytes: usize,
    current_bytes: usize,
}

impl DefaultBatchStrategy {
    pub fn new(max_batch_size: usize, max_batch_bytes: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_batch_size,
            max_batch_bytes,
            current_bytes: 0,
        }
    }
}

impl BatchStrategy for DefaultBatchStrategy {
    fn push(&mut self, envelope: IngestionEnvelope) {
        if let Ok(s) = serde_json::to_string(&envelope) {
            self.current_bytes += s.len();
        }
        self.buffer.push(envelope);
    }

    fn should_flush(&self) -> bool {
        self.buffer.len() >= self.max_batch_size || self.current_bytes >= self.max_batch_bytes
    }

    fn drain(&mut self) -> Vec<IngestionEnvelope> {
        self.current_bytes = 0;
        std::mem::take(&mut self.buffer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Disconnected,
    Connecting,
    Connected,
    Replaying,
    Ready,
}

pub struct BrainClient {
    tx: mpsc::Sender<ClientCommand>,
    config: Arc<ClientConfig>,
    state: Arc<std::sync::Mutex<RuntimeState>>,
    last_sequence: Arc<std::sync::atomic::AtomicU64>,
    last_sequence_received: Arc<std::sync::atomic::AtomicU64>,
    replay_strategy: Arc<dyn ReplayStrategy>,
    subscribers: Arc<
        std::sync::Mutex<Vec<mpsc::UnboundedSender<v1::StreamMessage>>>,
    >,
}

impl BrainClient {
    pub async fn connect(config: ClientConfig) -> Result<Self, BrainSdkError> {
        let (tx, rx) = mpsc::channel(config.max_queued_events);
        let config_arc = Arc::new(config);
        let state = Arc::new(std::sync::Mutex::new(RuntimeState::Disconnected));
        let last_sequence = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let last_sequence_received = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let replay_strategy = Arc::new(InMemoryReplayStrategy::new());
        let subscribers = Arc::new(std::sync::Mutex::new(Vec::new()));

        let runtime = ClientRuntime::new(
            Arc::clone(&config_arc),
            rx,
            Arc::clone(&state),
            Arc::clone(&last_sequence),
            Arc::clone(&last_sequence_received),
            Arc::clone(&replay_strategy) as Arc<dyn ReplayStrategy>,
            Arc::clone(&subscribers),
        );
        tokio::spawn(async move {
            runtime.run().await;
        });

        Ok(Self {
            tx,
            config: config_arc,
            state,
            last_sequence,
            last_sequence_received,
            replay_strategy,
            subscribers,
        })
    }

    pub fn state(&self) -> RuntimeState {
        *self.state.lock().unwrap()
    }

    pub fn last_sequence(&self) -> u64 {
        self.last_sequence
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn last_sequence_received(&self) -> u64 {
        self.last_sequence_received
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn get_unacknowledged_events(&self) -> Vec<IngestionEnvelope> {
        self.replay_strategy.get_unacknowledged().await
    }

    pub async fn request_replay(
        &self,
        after_sequence: u64,
    ) -> Result<Vec<IngestionEnvelope>, BrainSdkError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ClientCommand::Replay {
                after_sequence,
                tx: reply_tx,
            })
            .await
            .map_err(|_| BrainSdkError::ShuttingDown)?;
        reply_rx.await.map_err(|_| BrainSdkError::ShuttingDown)?
    }

    pub async fn send(&self, event: IngestionEvent) -> Result<IngestAck, BrainSdkError> {
        let envelope = IngestionEnvelope {
            event_model_version: "1.0".to_string(),
            identity: EventIdentity {
                event_id: EventId::new(),
                parent_event_id: None,
                workspace_id: self.config.workspace_id.clone(),
                client_id: self.config.client_id.clone(),
                adapter_id: self.config.adapter_id.clone(),
                session_id: self.config.session_id,
                conversation_id: self.config.conversation_id,
                timestamp: chrono::Utc::now(),
            },
            event,
        };

        let (reply_tx, reply_rx) = oneshot::channel();

        match self.config.backpressure_policy {
            BackpressurePolicy::Block => {
                self.tx
                    .send(ClientCommand::Send {
                        event: envelope.event,
                        tx: reply_tx,
                    })
                    .await
                    .map_err(|_| BrainSdkError::ShuttingDown)?;
            }
            BackpressurePolicy::Fail => {
                if self.tx.capacity() == 0 {
                    return Err(BrainSdkError::QueueFull);
                }
                self.tx
                    .send(ClientCommand::Send {
                        event: envelope.event,
                        tx: reply_tx,
                    })
                    .await
                    .map_err(|_| BrainSdkError::ShuttingDown)?;
            }
            BackpressurePolicy::DropOldest => {
                // DropOldest would normally pop from channel, but since standard mpsc channel
                // doesn't support pop_front, we block. Or we fail. For now, let's treat as Block/Fail.
                self.tx
                    .send(ClientCommand::Send {
                        event: envelope.event,
                        tx: reply_tx,
                    })
                    .await
                    .map_err(|_| BrainSdkError::ShuttingDown)?;
            }
        }

        reply_rx.await.map_err(|_| BrainSdkError::ShuttingDown)?
    }

    pub async fn status(&self) -> Result<v1::Status, BrainSdkError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ClientCommand::Rpc {
                action: "v1/status".to_string(),
                body: "".to_string(),
                tx: reply_tx,
            })
            .await
            .map_err(|_| BrainSdkError::ShuttingDown)?;
        let resp = reply_rx.await.map_err(|_| BrainSdkError::ShuttingDown)??;
        serde_json::from_str(&resp).map_err(|e| BrainSdkError::Serialization(e.to_string()))
     }

    pub async fn metrics(&self) -> Result<v1::Metrics, BrainSdkError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ClientCommand::Rpc {
                action: "v1/metrics".to_string(),
                body: "".to_string(),
                tx: reply_tx,
            })
            .await
            .map_err(|_| BrainSdkError::ShuttingDown)?;
        let resp = reply_rx.await.map_err(|_| BrainSdkError::ShuttingDown)??;
        serde_json::from_str(&resp).map_err(|e| BrainSdkError::Serialization(e.to_string()))
    }

    pub async fn diagnostics(
        &self,
    ) -> Result<v1::Diagnostics, BrainSdkError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ClientCommand::Rpc {
                action: "v1/diagnostics".to_string(),
                body: "".to_string(),
                tx: reply_tx,
            })
            .await
            .map_err(|_| BrainSdkError::ShuttingDown)?;
        let resp = reply_rx.await.map_err(|_| BrainSdkError::ShuttingDown)??;
        serde_json::from_str(&resp).map_err(|e| BrainSdkError::Serialization(e.to_string()))
    }

    pub async fn capabilities(
        &self,
    ) -> Result<Vec<v1::Capability>, BrainSdkError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ClientCommand::Rpc {
                action: "v1/capabilities".to_string(),
                body: "".to_string(),
                tx: reply_tx,
            })
            .await
            .map_err(|_| BrainSdkError::ShuttingDown)?;
        let resp = reply_rx.await.map_err(|_| BrainSdkError::ShuttingDown)??;
        serde_json::from_str(&resp).map_err(|e| BrainSdkError::Serialization(e.to_string()))
    }

    pub async fn search(
        &self,
        query: v1::SearchQuery,
    ) -> Result<Vec<v1::SearchSummary>, BrainSdkError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let query_str = serde_json::to_string(&query)
            .map_err(|e| BrainSdkError::Serialization(e.to_string()))?;
        self.tx
            .send(ClientCommand::Rpc {
                action: "v1/search".to_string(),
                body: query_str,
                tx: reply_tx,
            })
            .await
            .map_err(|_| BrainSdkError::ShuttingDown)?;
        let resp = reply_rx.await.map_err(|_| BrainSdkError::ShuttingDown)??;
        serde_json::from_str(&resp).map_err(|e| BrainSdkError::Serialization(e.to_string()))
    }

    pub async fn reflect(&self) -> Result<v1::ReflectionReport, BrainSdkError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ClientCommand::Rpc {
                action: "v1/reflect".to_string(),
                body: "".to_string(),
                tx: reply_tx,
            })
            .await
            .map_err(|_| BrainSdkError::ShuttingDown)?;
        let resp = reply_rx.await.map_err(|_| BrainSdkError::ShuttingDown)??;
        serde_json::from_str(&resp).map_err(|e| BrainSdkError::Serialization(e.to_string()))
    }

    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<v1::StreamMessage> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.subscribers.lock().unwrap().push(tx);
        rx
    }

    pub async fn shutdown(&self) {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(ClientCommand::Shutdown { tx }).await;
        let _ = rx.await;
    }
}

struct ClientRuntime {
    config: Arc<ClientConfig>,
    rx: mpsc::Receiver<ClientCommand>,
    state: Arc<std::sync::Mutex<RuntimeState>>,
    last_sequence: Arc<std::sync::atomic::AtomicU64>,
    last_sequence_received: Arc<std::sync::atomic::AtomicU64>,
    replay_strategy: Arc<dyn ReplayStrategy>,
    batch_strategy: Box<dyn BatchStrategy>,
    pending_acks: BTreeMap<EventId, oneshot::Sender<Result<IngestAck, BrainSdkError>>>,
    pending_replay_tx: Option<oneshot::Sender<Result<Vec<IngestionEnvelope>, BrainSdkError>>>,
    pending_requests: BTreeMap<u64, oneshot::Sender<Result<String, BrainSdkError>>>,
    request_id_counter: u64,
    subscribers: Arc<
        std::sync::Mutex<Vec<mpsc::UnboundedSender<v1::StreamMessage>>>,
    >,
}

impl ClientRuntime {
    fn new(
        config: Arc<ClientConfig>,
        rx: mpsc::Receiver<ClientCommand>,
        state: Arc<std::sync::Mutex<RuntimeState>>,
        last_sequence: Arc<std::sync::atomic::AtomicU64>,
        last_sequence_received: Arc<std::sync::atomic::AtomicU64>,
        replay_strategy: Arc<dyn ReplayStrategy>,
        subscribers: Arc<
            std::sync::Mutex<Vec<mpsc::UnboundedSender<v1::StreamMessage>>>,
        >,
    ) -> Self {
        let batch_strategy = Box::new(DefaultBatchStrategy::new(
            config.max_batch_size,
            config.max_batch_bytes,
        ));
        Self {
            config,
            rx,
            state,
            last_sequence,
            last_sequence_received,
            replay_strategy,
            batch_strategy,
            pending_acks: BTreeMap::new(),
            pending_replay_tx: None,
            pending_requests: BTreeMap::new(),
            request_id_counter: 0,
            subscribers,
        }
    }

    fn transition_to(&mut self, new_state: RuntimeState) {
        let mut guard = self.state.lock().unwrap();
        println!(
            "[SDK STATE] Transitioning from {:?} to {:?}",
            *guard, new_state
        );
        *guard = new_state;
    }

    fn handle_disconnect(&mut self) {
        self.transition_to(RuntimeState::Disconnected);
        let keys: Vec<EventId> = self.pending_acks.keys().cloned().collect();
        for key in keys {
            if let Some(tx) = self.pending_acks.remove(&key) {
                let _ = tx.send(Err(BrainSdkError::ConnectionFailed(
                    "Disconnected".to_string(),
                )));
            }
        }
        if let Some(tx) = self.pending_replay_tx.take() {
            let _ = tx.send(Err(BrainSdkError::ConnectionFailed(
                "Disconnected".to_string(),
            )));
        }
        let req_keys: Vec<u64> = self.pending_requests.keys().cloned().collect();
        for key in req_keys {
            if let Some(tx) = self.pending_requests.remove(&key) {
                let _ = tx.send(Err(BrainSdkError::ConnectionFailed(
                    "Disconnected".to_string(),
                )));
            }
        }
    }

    async fn run(mut self) {
        let mut flush_timer = tokio::time::interval(self.config.flush_interval);
        let mut heartbeat_timer = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(5),
            Duration::from_secs(5),
        );
        let start_time = Instant::now();

        let mut reconnect_delay = Duration::from_millis(100);
        let mut reconnect_timer = Box::pin(tokio::time::sleep(Duration::from_millis(0)));
        let mut is_connecting = false;

        let mut write_half: Option<tokio::net::unix::OwnedWriteHalf> = None;
        let mut buf_reader: Option<BufReader<tokio::net::unix::OwnedReadHalf>> = None;
        let mut line = String::new();

        loop {
            if write_half.is_none() && !is_connecting {
                self.handle_disconnect();
                reconnect_timer = Box::pin(tokio::time::sleep(reconnect_delay));
                is_connecting = true;
            }

            tokio::select! {
                _ = &mut reconnect_timer, if write_half.is_none() && is_connecting => {
                    self.transition_to(RuntimeState::Connecting);
                    match UnixStream::connect(&self.config.socket_path).await {
                        Ok(stream) => {
                            println!("[SDK UDS] Connected to daemon at {:?}", self.config.socket_path);
                            let (r, w) = stream.into_split();
                            write_half = Some(w);
                            buf_reader = Some(BufReader::new(r));
                            is_connecting = false;
                            reconnect_delay = Duration::from_millis(100);

                            // Perform Handshake
                            self.transition_to(RuntimeState::Connecting);
                            let handshake_req = serde_json::json!({
                                "protocol_version": "1.0",
                                "connection_id": uuid::Uuid::new_v4().to_string(),
                                "adapter_id": self.config.adapter_id.to_string(),
                                "session_id": self.config.session_id.to_string(),
                                "capabilities": vec!["ConversationMessages", "WorkspaceFiles"],
                            });
                            let handshake_wire = serde_json::json!({
                                "action": "handshake",
                                "payload": serde_json::to_string(&handshake_req).unwrap()
                            });
                            let mut handshake_str = serde_json::to_string(&handshake_wire).unwrap();
                            handshake_str.push('\n');

                            let mut handshake_ok = false;
                            if let Some(w_ref) = write_half.as_mut() {
                                if w_ref.write_all(handshake_str.as_bytes()).await.is_ok() && w_ref.flush().await.is_ok() {
                                    let mut hs_line = String::new();
                                    if let Some(r_ref) = buf_reader.as_mut() {
                                        if r_ref.read_line(&mut hs_line).await.is_ok() {
                                            if let Ok(resp_json) = serde_json::from_str::<serde_json::Value>(&hs_line) {
                                                if resp_json.get("status").is_some_and(|s| s == "ok" || s == "success") {
                                                    println!("[SDK UDS] Handshake successful!");
                                                    handshake_ok = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if !handshake_ok {
                                println!("[SDK UDS] Handshake failed. Disconnecting.");
                                write_half = None;
                                buf_reader = None;
                                self.handle_disconnect();
                                continue;
                            }

                            self.transition_to(RuntimeState::Replaying);
                            let unacknowledged = self.replay_strategy.get_unacknowledged().await;
                            let mut replay_failed = false;
                            if let Some(w_ref) = write_half.as_mut() {
                                for envelope in unacknowledged {
                                    if let Ok(json_str) = brain_integrations::to_canonical_json(&envelope) {
                                        let request = serde_json::json!({
                                            "action": "ingest_event",
                                            "payload": json_str
                                        });
                                        if let Ok(mut wire_str) = serde_json::to_string(&request) {
                                            wire_str.push('\n');
                                            if let Err(e) = w_ref.write_all(wire_str.as_bytes()).await {
                                                println!("[SDK UDS] Replay write failed: {}", e);
                                                replay_failed = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            if replay_failed {
                                write_half = None;
                                buf_reader = None;
                                self.handle_disconnect();
                            } else {
                                self.transition_to(RuntimeState::Ready);
                                if let Some(w_ref) = write_half.as_mut() {
                                    let last_seq = self.last_sequence_received.load(std::sync::atomic::Ordering::Relaxed);
                                    let sub_req = if last_seq > 0 {
                                        serde_json::json!({
                                            "after_sequence": last_seq
                                        })
                                    } else {
                                        serde_json::json!({})
                                    };
                                    let request = serde_json::json!({
                                        "version": "1.0",
                                        "type": "Request",
                                        "id": 0,
                                        "action": "v1/subscribe",
                                        "body": serde_json::to_string(&sub_req).unwrap()
                                    });
                                    if let Ok(mut wire_str) = serde_json::to_string(&request) {
                                        wire_str.push('\n');
                                        let _ = w_ref.write_all(wire_str.as_bytes()).await;
                                        let _ = w_ref.flush().await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("[SDK UDS] Failed to connect: {}. Will retry...", e);
                            is_connecting = false;
                            let jitter = (rand::random::<f64>() - 0.5) * 0.2;
                            reconnect_delay = Duration::from_millis(((reconnect_delay.as_millis() as f64 * 2.0) * (1.0 + jitter)) as u64);
                            if reconnect_delay > Duration::from_secs(10) {
                                reconnect_delay = Duration::from_secs(10);
                            }
                        }
                    }
                }

                cmd_opt = self.rx.recv() => {
                    match cmd_opt {
                        Some(ClientCommand::Send { event, tx }) => {
                            let envelope = IngestionEnvelope {
                                event_model_version: "1.0".to_string(),
                                identity: EventIdentity {
                                    event_id: EventId::new(),
                                    parent_event_id: None,
                                    workspace_id: self.config.workspace_id.clone(),
                                    client_id: self.config.client_id.clone(),
                                    adapter_id: self.config.adapter_id.clone(),
                                    session_id: self.config.session_id,
                                    conversation_id: self.config.conversation_id,
                                    timestamp: chrono::Utc::now(),
                                },
                                event,
                            };

                            let event_id = envelope.identity.event_id;
                            let _ = self.replay_strategy.record(envelope.clone()).await;
                            self.pending_acks.insert(event_id, tx);

                            self.batch_strategy.push(envelope);

                            println!("[SDK BATCH] Pushed event. should_flush={}", self.batch_strategy.should_flush());
                            if self.batch_strategy.should_flush() {
                                if let Some(w_ref) = write_half.as_mut() {
                                    let batch = self.batch_strategy.drain();
                                    println!("[SDK BATCH] Flushing batch of size {}", batch.len());
                                    let mut write_failed = false;
                                    for envelope in batch {
                                        if let Ok(json_str) = brain_integrations::to_canonical_json(&envelope) {
                                            let request = serde_json::json!({
                                                "action": "ingest_event",
                                                "payload": json_str
                                            });
                                            if let Ok(mut wire_str) = serde_json::to_string(&request) {
                                                wire_str.push('\n');
                                                if let Err(e) = w_ref.write_all(wire_str.as_bytes()).await {
                                                    println!("[SDK UDS] Batch write failed: {}", e);
                                                    write_failed = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    if write_failed {
                                        write_half = None;
                                        buf_reader = None;
                                        self.handle_disconnect();
                                    } else {
                                        let _ = w_ref.flush().await;
                                    }
                                }
                            }
                        }
                        Some(ClientCommand::Replay { after_sequence, tx }) => {
                            if let Some(w_ref) = write_half.as_mut() {
                                let request = serde_json::json!({
                                    "action": "replay",
                                    "payload": after_sequence.to_string()
                                });
                                if let Ok(mut wire_str) = serde_json::to_string(&request) {
                                    wire_str.push('\n');
                                    if let Err(e) = w_ref.write_all(wire_str.as_bytes()).await {
                                        let _ = tx.send(Err(BrainSdkError::ConnectionFailed(e.to_string())));
                                        write_half = None;
                                        buf_reader = None;
                                        self.handle_disconnect();
                                    } else {
                                        let _ = w_ref.flush().await;
                                        self.pending_replay_tx = Some(tx);
                                    }
                                } else {
                                    let _ = tx.send(Err(BrainSdkError::SendError("Serialization failed".to_string())));
                                }
                            } else {
                                let _ = tx.send(Err(BrainSdkError::ConnectionFailed("Not connected".to_string())));
                            }
                        }
                        Some(ClientCommand::Rpc { action, body, tx }) => {
                            if let Some(w_ref) = write_half.as_mut() {
                                self.request_id_counter += 1;
                                let req_id = self.request_id_counter;
                                let request = serde_json::json!({
                                    "version": "1.0",
                                    "type": "Request",
                                    "id": req_id,
                                    "action": action,
                                    "body": body
                                });
                                if let Ok(mut wire_str) = serde_json::to_string(&request) {
                                    wire_str.push('\n');
                                    if let Err(e) = w_ref.write_all(wire_str.as_bytes()).await {
                                        let _ = tx.send(Err(BrainSdkError::ConnectionFailed(e.to_string())));
                                        write_half = None;
                                        buf_reader = None;
                                        self.handle_disconnect();
                                    } else {
                                        let _ = w_ref.flush().await;
                                        self.pending_requests.insert(req_id, tx);
                                    }
                                } else {
                                    let _ = tx.send(Err(BrainSdkError::SendError("Serialization failed".to_string())));
                                }
                            } else {
                                let _ = tx.send(Err(BrainSdkError::ConnectionFailed("Not connected".to_string())));
                            }
                        }
                        Some(ClientCommand::Shutdown { tx }) => {
                            if let Some(w_ref) = write_half.as_mut() {
                                let batch = self.batch_strategy.drain();
                                for envelope in batch {
                                    if let Ok(json_str) = brain_integrations::to_canonical_json(&envelope) {
                                        let request = serde_json::json!({
                                            "action": "ingest_event",
                                            "payload": json_str
                                        });
                                        if let Ok(mut wire_str) = serde_json::to_string(&request) {
                                            wire_str.push('\n');
                                            let _ = w_ref.write_all(wire_str.as_bytes()).await;
                                        }
                                    }
                                }
                                // Send disconnect action frame
                                let disc_req = serde_json::json!({
                                    "reason": "clean_exit",
                                    "last_sent_sequence": self.last_sequence.load(std::sync::atomic::Ordering::Relaxed)
                                });
                                let disc_wire = serde_json::json!({
                                    "action": "disconnect",
                                    "payload": serde_json::to_string(&disc_req).unwrap()
                                });
                                if let Ok(mut wire_str) = serde_json::to_string(&disc_wire) {
                                    wire_str.push('\n');
                                    let _ = w_ref.write_all(wire_str.as_bytes()).await;
                                }
                                let _ = w_ref.flush().await;
                            }
                            let _ = tx.send(());
                            return;
                        }
                        None => return,
                    }
                }

                _ = heartbeat_timer.tick(), if write_half.is_some() => {
                    if let Some(w_ref) = write_half.as_mut() {
                        let queue_len = self.replay_strategy.get_unacknowledged().await.len();
                        let heartbeat_req = serde_json::json!({
                            "last_ack_sequence": self.last_sequence.load(std::sync::atomic::Ordering::Relaxed),
                            "queue_depth": queue_len,
                            "pending_batches": 0,
                            "uptime_ms": start_time.elapsed().as_millis() as u64,
                        });
                        let heartbeat_wire = serde_json::json!({
                            "action": "heartbeat",
                            "payload": serde_json::to_string(&heartbeat_req).unwrap()
                        });
                        if let Ok(mut wire_str) = serde_json::to_string(&heartbeat_wire) {
                            wire_str.push('\n');
                            if let Err(e) = w_ref.write_all(wire_str.as_bytes()).await {
                                println!("[SDK UDS] Heartbeat write failed: {}", e);
                                write_half = None;
                                buf_reader = None;
                                self.handle_disconnect();
                            } else {
                                let _ = w_ref.flush().await;
                            }
                        }
                    }
                }

                _ = flush_timer.tick() => {
                    let batch = self.batch_strategy.drain();
                    if !batch.is_empty() {
                        if let Some(w_ref) = write_half.as_mut() {
                            println!("[SDK TIMER] Drained flush batch of size {}", batch.len());
                            let mut write_failed = false;
                            for envelope in batch {
                                if let Ok(json_str) = brain_integrations::to_canonical_json(&envelope) {
                                    let request = serde_json::json!({
                                        "action": "ingest_event",
                                        "payload": json_str
                                    });
                                    if let Ok(mut wire_str) = serde_json::to_string(&request) {
                                        wire_str.push('\n');
                                        if let Err(e) = w_ref.write_all(wire_str.as_bytes()).await {
                                            println!("[SDK UDS] Flush write failed: {}", e);
                                            write_failed = true;
                                            break;
                                        }
                                    }
                                }
                            }
                            if write_failed {
                                write_half = None;
                                buf_reader = None;
                                self.handle_disconnect();
                            } else {
                                let _ = w_ref.flush().await;
                            }
                        }
                    }
                }

                res = read_line_opt(buf_reader.as_mut(), &mut line), if buf_reader.is_some() => {
                    match res {
                        Ok(0) => {
                            println!("[SDK UDS] Daemon socket closed (EOF).");
                            write_half = None;
                            buf_reader = None;
                            self.handle_disconnect();
                        }
                        Ok(_) => {
                            let resp_str = line.trim();
                            println!("[SDK UDS] Read line: {}", resp_str);
                            if !resp_str.is_empty() {
                                if let Ok(resp_json) = serde_json::from_str::<serde_json::Value>(resp_str) {
                                    println!("[SDK UDS] Parsed JSON: {:?}", resp_json);

                                    if resp_json.get("type").and_then(|t| t.as_str()).is_some_and(|t| t == "Event") {
                                        let payload = resp_json.get("payload").cloned().unwrap_or(serde_json::Value::Null);
                                        if let Ok(msg) = serde_json::from_value::<v1::StreamMessage>(payload) {
                                            if let v1::StreamMessage::Event { sequence, .. } = &msg {
                                                self.last_sequence_received.store(*sequence, std::sync::atomic::Ordering::Relaxed);
                                            }
                                            let mut subs = self.subscribers.lock().unwrap();
                                            subs.retain(|sub| {
                                                sub.send(msg.clone()).is_ok()
                                            });
                                        }
                                        line.clear();
                                        continue;
                                    }

                                    if let Some(req_id) = resp_json.get("id").and_then(|id| id.as_u64()) {
                                        if let Some(tx) = self.pending_requests.remove(&req_id) {
                                            let is_error = resp_json.get("type").and_then(|t| t.as_str()).is_some_and(|t| t == "Error")
                                                || resp_json.get("status").and_then(|s| s.as_str()).is_some_and(|s| s == "error");
                                            let body_str = resp_json.get("body")
                                                .and_then(|b| b.as_str())
                                                .unwrap_or("");
                                            if is_error {
                                                let _ = tx.send(Err(BrainSdkError::DaemonError(body_str.to_string())));
                                            } else {
                                                let _ = tx.send(Ok(body_str.to_string()));
                                            }
                                            line.clear();
                                            continue;
                                        }
                                    }

                                    let body_str = resp_json.get("body")
                                        .and_then(|b| b.as_str())
                                        .or_else(|| resp_json.get("message").and_then(|m| m.as_str()));

                                    println!("[SDK UDS] Extracted body/message: {:?}", body_str);
                                    if let Some(body) = body_str {
                                        if let Ok(ack) = serde_json::from_str::<IngestAck>(body) {
                                            println!("[SDK UDS] Successfully parsed IngestAck: {:?}", ack);
                                            self.last_sequence.store(ack.sequence, std::sync::atomic::Ordering::Relaxed);
                                            let _ = self.replay_strategy.acknowledge(&ack.event_id).await;
                                            if let Some(tx) = self.pending_acks.remove(&ack.event_id) {
                                                let _ = tx.send(Ok(ack));
                                            }
                                        } else if let Ok(events) = serde_json::from_str::<Vec<IngestionEnvelope>>(body) {
                                            println!("[SDK UDS] Successfully parsed Replay response with {} events", events.len());
                                            if let Some(tx) = self.pending_replay_tx.take() {
                                                let _ = tx.send(Ok(events));
                                            }
                                        } else {
                                            println!("[SDK UDS] Failed to parse body as IngestAck or Vec<IngestionEnvelope>: {}", body);
                                        }
                                    }
                                }
                            }
                            line.clear();
                        }
                        Err(e) => {
                            println!("[SDK UDS] Read error: {}", e);
                            write_half = None;
                            buf_reader = None;
                            self.handle_disconnect();
                        }
                    }
                }
            }
        }
    }
}

async fn read_line_opt(
    reader: Option<&mut BufReader<tokio::net::unix::OwnedReadHalf>>,
    line: &mut String,
) -> std::io::Result<usize> {
    if let Some(r) = reader {
        r.read_line(line).await
    } else {
        std::future::pending().await
    }
}
