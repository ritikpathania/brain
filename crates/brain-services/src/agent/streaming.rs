use crate::agent::{
    AgentExecutionEvent, AgentExecutionEventPayload, ExecutionId, ExecutionMetrics,
};
use std::time::{Duration, SystemTime};

/// Configurable overflow policy for bounded event queues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum OverflowPolicy {
    /// Drop the oldest progress update inside the queue.
    SelectiveDrop,
    /// Drop the newly incoming event if full.
    DropNewest,
    /// Drop the oldest event in the queue if full.
    DropOldest,
}

/// The state classification of a runner execution stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum StageStatus {
    /// Stage has started execution.
    Started,
    /// Stage has successfully or unsuccessfully completed.
    Completed,
}

/// The status of a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ToolStatus {
    /// Tool invocation has commenced.
    Invoked,
    /// Tool has finished successfully.
    Completed,
    /// Tool execution failed.
    Failed,
}

/// Immutable record of a single stage's timing metrics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TimelineEntry {
    /// The global monotonic sequence ID when the entry completed.
    pub sequence: u64,
    /// Name of the stage.
    pub stage: &'static str,
    /// Exact SystemTime the stage started.
    pub started_at: SystemTime,
    /// Exact SystemTime the stage completed.
    pub finished_at: SystemTime,
    /// Computed duration of execution.
    pub duration: Duration,
}

/// Event payload carrying streamed token chunks.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenEvent {
    /// Chunk of generated text response.
    pub token: String,
}

/// Event payload representing generic execution progress logs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProgressEvent {
    /// Descriptive human-readable log message.
    pub message: String,
    /// Optional monotonic percentage of completion progress.
    pub percentage: Option<f32>,
}

/// Event payload tracking a runner stage transition.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StageEvent {
    /// Name of the stage.
    pub stage: &'static str,
    /// Updated status.
    pub status: StageStatus,
}

/// Event payload tracking a tool call transition.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolEvent {
    /// Name of the tool invoked.
    pub tool_name: String,
    /// Transition status.
    pub status: ToolStatus,
}

/// Event payload containing a completed timeline entry snapshot.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TimelineEvent {
    /// Timeline entry.
    pub entry: TimelineEntry,
}

/// Event payload indicating successful execution completion.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FinishedEvent {
    /// The final generated assistant response text.
    pub response: String,
    /// Collected telemetry and token usage metrics.
    pub metrics: ExecutionMetrics,
}

/// Event payload indicating cancellation by subscriber or consumer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CancelledEvent {}

/// Event payload carrying error diagnostic information.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorEvent {
    /// Human-readable error message.
    pub message: String,
}

/// Strongly typed categories of runtime stream events.
#[derive(Debug, Clone, serde::Serialize)]
pub enum StreamEventPayload {
    /// Text token generated.
    Token(TokenEvent),
    /// Diagnostic progress log update.
    Progress(ProgressEvent),
    /// Runner stage transition update.
    Stage(StageEvent),
    /// Tool call transition update.
    Tool(ToolEvent),
    /// Timeline record generated.
    Timeline(TimelineEvent),
    /// Successful execution final report.
    Finished(FinishedEvent),
    /// Cancellation acknowledgement.
    Cancelled(CancelledEvent),
    /// Unsuccessful execution error report.
    Error(ErrorEvent),
}

/// Globally sequence-ordered event envelope carrying structural metadata.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamEvent {
    /// Unique execution identifier.
    pub execution_id: ExecutionId,
    /// Monotonically increasing sequence ID.
    pub sequence: u64,
    /// Exact SystemTime the event was constructed in the event sink.
    pub timestamp: SystemTime,
    /// Category payload.
    pub payload: StreamEventPayload,
}

impl StreamEvent {
    /// Returns true if the event payload carries an essential token or final outcome.
    pub fn is_essential(&self) -> bool {
        matches!(
            &self.payload,
            StreamEventPayload::Token(_)
                | StreamEventPayload::Finished(_)
                | StreamEventPayload::Cancelled(_)
                | StreamEventPayload::Error(_)
                | StreamEventPayload::Tool(_)
        )
    }
}

/// Pluggable mapping adapter interface transforming runner events into user-facing stream events.
pub trait StreamEventMapper: Send + Sync {
    /// Maps a raw internal execution event into a user-facing stream event.
    fn map(&self, event: AgentExecutionEvent) -> StreamEvent;
}

/// Default implementation mapping standard execution events.
pub struct DefaultStreamEventMapper;

impl StreamEventMapper for DefaultStreamEventMapper {
    fn map(&self, event: AgentExecutionEvent) -> StreamEvent {
        let payload = match event.payload {
            AgentExecutionEventPayload::ExecutionStarted { prompt, .. } => {
                StreamEventPayload::Progress(ProgressEvent {
                    message: format!("Execution started: {}", prompt),
                    percentage: Some(0.0),
                })
            }
            AgentExecutionEventPayload::PlanningStarted { .. } => {
                StreamEventPayload::Stage(StageEvent {
                    stage: "Planning",
                    status: StageStatus::Started,
                })
            }
            AgentExecutionEventPayload::RetrievalCompleted { .. } => {
                StreamEventPayload::Stage(StageEvent {
                    stage: "Retrieval",
                    status: StageStatus::Completed,
                })
            }
            AgentExecutionEventPayload::StageStarted { stage, .. } => {
                StreamEventPayload::Stage(StageEvent {
                    stage,
                    status: StageStatus::Started,
                })
            }
            AgentExecutionEventPayload::StageCompleted { stage, .. } => {
                StreamEventPayload::Stage(StageEvent {
                    stage,
                    status: StageStatus::Completed,
                })
            }
            AgentExecutionEventPayload::TokenStreamed { token, .. } => {
                StreamEventPayload::Token(TokenEvent { token })
            }
            AgentExecutionEventPayload::ExecutionFinished { response, .. } => {
                StreamEventPayload::Finished(FinishedEvent {
                    response,
                    metrics: ExecutionMetrics::default(),
                })
            }
            AgentExecutionEventPayload::ExecutionFailed { error, .. } => {
                StreamEventPayload::Error(ErrorEvent { message: error })
            }
            AgentExecutionEventPayload::ExecutionCancelled { .. } => {
                StreamEventPayload::Cancelled(CancelledEvent {})
            }
            _ => StreamEventPayload::Progress(ProgressEvent {
                message: "Activity recorded".to_string(),
                percentage: None,
            }),
        };

        StreamEvent {
            execution_id: event.execution_id,
            sequence: event.sequence,
            timestamp: event.timestamp,
            payload,
        }
    }
}

/// A bounded, thread-safe queue with configurable overflow policies.
pub struct SafeEventQueue {
    capacity: usize,
    policy: OverflowPolicy,
    inner: parking_lot::Mutex<EventQueueInner>,
    waker: tokio::sync::Notify,
}

struct EventQueueInner {
    queue: std::collections::VecDeque<StreamEvent>,
    finished: bool,
}

impl SafeEventQueue {
    /// Creates a new SafeEventQueue with given capacity and overflow policy.
    pub fn new(capacity: usize, policy: OverflowPolicy) -> Self {
        Self {
            capacity,
            policy,
            inner: parking_lot::Mutex::new(EventQueueInner {
                queue: std::collections::VecDeque::new(),
                finished: false,
            }),
            waker: tokio::sync::Notify::new(),
        }
    }

    /// Pushes an event onto the queue using the overflow policy boundaries.
    pub fn push(&self, event: StreamEvent) {
        let mut inner = self.inner.lock();
        if inner.finished {
            return;
        }

        if inner.queue.len() >= self.capacity {
            match self.policy {
                OverflowPolicy::SelectiveDrop => {
                    if event.is_essential() {
                        // Soft-limit: drop oldest non-essential event if possible,
                        // otherwise exceed the capacity limit (essential events never drop)
                        if let Some(pos) = inner.queue.iter().position(|e| !e.is_essential()) {
                            inner.queue.remove(pos);
                            inner.queue.push_back(event);
                        } else {
                            inner.queue.push_back(event);
                        }
                    } else {
                        // Non-essential event: drop the oldest non-essential event in the queue,
                        // or drop self if the queue is entirely essential events
                        if let Some(pos) = inner.queue.iter().position(|e| !e.is_essential()) {
                            inner.queue.remove(pos);
                            inner.queue.push_back(event);
                        }
                    }
                }
                OverflowPolicy::DropOldest => {
                    inner.queue.pop_front();
                    inner.queue.push_back(event);
                }
                OverflowPolicy::DropNewest => {
                    // Drop incoming event (do nothing)
                }
            }
        } else {
            inner.queue.push_back(event);
        }
        self.waker.notify_one();
    }

    /// Closes the queue, preventing future pushes and waking any waiting receivers.
    pub fn close(&self) {
        let mut inner = self.inner.lock();
        inner.finished = true;
        self.waker.notify_one();
    }
}

/// Strongly typed subscriber identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct SubscriberId(uuid::Uuid);

impl SubscriberId {
    /// Generates a new unique SubscriberId.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

/// A wrapper mapping a subscriber queue to its ID.
pub struct SubscriberHandle {
    /// Unique subscriber ID.
    pub id: SubscriberId,
    /// Bounded event queue.
    pub queue: std::sync::Arc<SafeEventQueue>,
}

/// Registry of subscribers for a single execution.
pub struct ExecutionSubscribers {
    /// Active subscribers.
    pub subscribers: Vec<SubscriberHandle>,
    /// Accumulated event history for replay cursors.
    pub history: Vec<StreamEvent>,
}

impl ExecutionSubscribers {
    /// Creates an empty ExecutionSubscribers container.
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
            history: Vec::new(),
        }
    }
}

impl Default for ExecutionSubscribers {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulates stage start and end times to construct append-only timelines.
pub struct TimelineBuilder {
    starts: parking_lot::Mutex<
        std::collections::HashMap<ExecutionId, std::collections::HashMap<&'static str, SystemTime>>,
    >,
    entries: parking_lot::Mutex<std::collections::HashMap<ExecutionId, Vec<TimelineEntry>>>,
}

impl TimelineBuilder {
    /// Creates a new empty TimelineBuilder.
    pub fn new() -> Self {
        Self {
            starts: parking_lot::Mutex::new(std::collections::HashMap::new()),
            entries: parking_lot::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Processes an incoming event and returns a new derived timeline entry if a stage finishes.
    pub fn process_event(&self, event: &StreamEvent) -> Option<TimelineEntry> {
        if let StreamEventPayload::Stage(stage_evt) = &event.payload {
            let mut lock_starts = self.starts.lock();
            match stage_evt.status {
                StageStatus::Started => {
                    lock_starts
                        .entry(event.execution_id)
                        .or_default()
                        .insert(stage_evt.stage, event.timestamp);
                    None
                }
                StageStatus::Completed => {
                    if let Some(start) = lock_starts
                        .get_mut(&event.execution_id)
                        .and_then(|m| m.remove(stage_evt.stage))
                    {
                        let duration = event
                            .timestamp
                            .duration_since(start)
                            .unwrap_or(Duration::ZERO);

                        let entry = TimelineEntry {
                            sequence: event.sequence,
                            stage: stage_evt.stage,
                            started_at: start,
                            finished_at: event.timestamp,
                            duration,
                        };

                        let mut lock_entries = self.entries.lock();
                        lock_entries
                            .entry(event.execution_id)
                            .or_default()
                            .push(entry.clone());
                        Some(entry)
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        }
    }

    /// Returns a snapshot of the current timeline for an execution.
    pub fn timeline(&self, execution_id: ExecutionId) -> Vec<TimelineEntry> {
        self.entries
            .lock()
            .get(&execution_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Cleans up state for a completed execution.
    pub fn cleanup(&self, execution_id: ExecutionId) {
        self.starts.lock().remove(&execution_id);
        self.entries.lock().remove(&execution_id);
    }
}

impl Default for TimelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulates telemetry metrics dynamically from the event stream.
pub struct MetricsCollector {
    metrics: parking_lot::Mutex<std::collections::HashMap<ExecutionId, ExecutionMetrics>>,
}

impl MetricsCollector {
    /// Creates a new empty MetricsCollector.
    pub fn new() -> Self {
        Self {
            metrics: parking_lot::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Updates internal metrics based on the incoming stream event.
    pub fn process_event(&self, event: &StreamEvent) {
        let mut lock = self.metrics.lock();
        let m = lock.entry(event.execution_id).or_default();

        match &event.payload {
            StreamEventPayload::Token(tok) => {
                m.tokens_used += tok.token.split_whitespace().count().max(1);
            }
            StreamEventPayload::Stage(stg) if stg.status == StageStatus::Completed => {
                m.step_count += 1;
            }
            _ => {}
        }
    }

    /// Returns a copy snapshot of the metrics for an execution.
    pub fn get_metrics(&self, execution_id: ExecutionId) -> Option<ExecutionMetrics> {
        self.metrics.lock().get(&execution_id).cloned()
    }

    /// Cleans up metrics registry for an execution.
    pub fn cleanup(&self, execution_id: ExecutionId) {
        self.metrics.lock().remove(&execution_id);
    }
}

/// Registry of subscribers facilitating broadcast loops.
pub struct SubscriberHub {
    pub(crate) subscribers:
        parking_lot::RwLock<std::collections::HashMap<ExecutionId, ExecutionSubscribers>>,
}

impl SubscriberHub {
    /// Creates a new empty SubscriberHub.
    pub fn new() -> Self {
        Self {
            subscribers: parking_lot::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Registers a new active execution ID.
    pub fn register_execution(&self, execution_id: ExecutionId) {
        self.subscribers
            .write()
            .insert(execution_id, ExecutionSubscribers::new());
    }

    /// Subscribes a new event queue to an execution.
    pub fn subscribe(
        &self,
        execution_id: ExecutionId,
        queue: std::sync::Arc<SafeEventQueue>,
    ) -> Option<SubscriberId> {
        let mut lock = self.subscribers.write();
        if let Some(state) = lock.get_mut(&execution_id) {
            let id = SubscriberId::new();
            // Pre-populate queue with history
            for evt in &state.history {
                queue.push(evt.clone());
            }
            state.subscribers.push(SubscriberHandle { id, queue });
            Some(id)
        } else {
            None
        }
    }

    /// Unsubscribes a subscriber handle by ID.
    pub fn unsubscribe(&self, execution_id: ExecutionId, id: SubscriberId) {
        let mut lock = self.subscribers.write();
        if let Some(state) = lock.get_mut(&execution_id) {
            state.subscribers.retain(|sub| sub.id != id);
        }
    }

    /// Broadcasts an event to all subscribers registered to the execution.
    pub fn broadcast(&self, execution_id: ExecutionId, event: StreamEvent) {
        let mut lock = self.subscribers.write();
        if let Some(state) = lock.get_mut(&execution_id) {
            state.history.push(event.clone());
            for sub in &state.subscribers {
                sub.queue.push(event.clone());
            }
        }
    }

    /// Closes all active subscriber queues for an execution.
    pub fn close_all(&self, execution_id: ExecutionId) {
        let mut lock = self.subscribers.write();
        if let Some(state) = lock.remove(&execution_id) {
            for sub in state.subscribers {
                sub.queue.close();
            }
        }
    }
}

impl Default for SubscriberHub {
    fn default() -> Self {
        Self::new()
    }
}

/// Private subscriber handle that handles automated unsubscription on drop.
pub struct ExecutionStream {
    pub(crate) id: SubscriberId,
    pub(crate) execution_id: ExecutionId,
    pub(crate) queue: std::sync::Arc<SafeEventQueue>,
    pub(crate) hub: std::sync::Arc<SubscriberHub>,
}

impl ExecutionStream {
    /// Returns the next event in the stream, or None if closed.
    pub async fn next(&self) -> Option<StreamEvent> {
        loop {
            let notify = self.queue.waker.notified();
            {
                let mut inner = self.queue.inner.lock();
                if let Some(event) = inner.queue.pop_front() {
                    return Some(event);
                }
                if inner.finished {
                    return None;
                }
            }
            notify.await;
        }
    }

    /// Helper test constructor to create a test execution stream.
    pub fn new_test(queue: std::sync::Arc<SafeEventQueue>, execution_id: ExecutionId) -> Self {
        Self {
            id: SubscriberId::new(),
            execution_id,
            queue,
            hub: std::sync::Arc::new(SubscriberHub::new()),
        }
    }
}

impl Drop for ExecutionStream {
    fn drop(&mut self) {
        self.hub.unsubscribe(self.execution_id, self.id);
    }
}

/// Unified active execution state container hiding synchronizations and exposing clean intent-based methods.
pub struct ExecutionRuntimeState {
    cancellation: std::sync::Arc<crate::agent::CancellationTokenImpl>,
    hub: std::sync::Arc<SubscriberHub>,
    timeline: std::sync::Arc<TimelineBuilder>,
    metrics: std::sync::Arc<MetricsCollector>,
}

impl ExecutionRuntimeState {
    /// Creates a new ExecutionRuntimeState.
    pub fn new(
        cancellation: std::sync::Arc<crate::agent::CancellationTokenImpl>,
        execution_id: ExecutionId,
    ) -> Self {
        let hub = std::sync::Arc::new(SubscriberHub::new());
        hub.register_execution(execution_id);
        Self {
            cancellation,
            hub,
            timeline: std::sync::Arc::new(TimelineBuilder::new()),
            metrics: std::sync::Arc::new(MetricsCollector::new()),
        }
    }

    /// Subscribes a new event stream to the execution.
    pub fn subscribe(&self, execution_id: ExecutionId) -> Option<ExecutionStream> {
        let queue = std::sync::Arc::new(SafeEventQueue::new(100, OverflowPolicy::SelectiveDrop));
        self.hub
            .subscribe(execution_id, queue.clone())
            .map(|id| ExecutionStream {
                id,
                execution_id,
                queue,
                hub: self.hub.clone(),
            })
    }

    /// Cancels execution.
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Returns timeline entries.
    pub fn timeline(&self, execution_id: ExecutionId) -> Vec<TimelineEntry> {
        self.timeline.timeline(execution_id)
    }

    /// Processes an incoming event.
    pub fn process_event(&self, event: &StreamEvent) -> Option<TimelineEntry> {
        self.metrics.process_event(event);
        self.timeline.process_event(event)
    }

    /// Broadcasts a mapped event to subscribers.
    pub fn broadcast(&self, execution_id: ExecutionId, event: StreamEvent) {
        self.hub.broadcast(execution_id, event);
    }

    /// Queries current metrics snapshot.
    pub fn metrics(&self, execution_id: ExecutionId) -> Option<ExecutionMetrics> {
        self.metrics.get_metrics(execution_id)
    }

    /// Performs dynamic resource cleanup.
    pub fn cleanup(&self, execution_id: ExecutionId) {
        self.metrics.cleanup(execution_id);
        self.timeline.cleanup(execution_id);
        self.hub.close_all(execution_id);
    }
}

/// The coordinator orchestrating mapping, broadcast, metrics, timelines, and cleanup states.
pub struct StreamingRuntime {
    states: std::sync::Arc<
        parking_lot::RwLock<
            std::collections::HashMap<ExecutionId, std::sync::Arc<ExecutionRuntimeState>>,
        >,
    >,
    mapper: std::sync::Arc<dyn StreamEventMapper>,
}

impl StreamingRuntime {
    /// Creates a new StreamingRuntime.
    pub fn new(mapper: std::sync::Arc<dyn StreamEventMapper>) -> Self {
        Self {
            states: std::sync::Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
            mapper,
        }
    }

    /// Registers a newly spawned execution, mapping and broadcasting its events in a background task.
    pub fn register(
        &self,
        execution_id: ExecutionId,
        mut event_rx: tokio::sync::mpsc::UnboundedReceiver<AgentExecutionEvent>,
        cancellation: std::sync::Arc<crate::agent::CancellationTokenImpl>,
    ) {
        let state = std::sync::Arc::new(ExecutionRuntimeState::new(cancellation, execution_id));
        self.states.write().insert(execution_id, state.clone());

        let mapper = self.mapper.clone();
        let states = self.states.clone();

        tokio::spawn(async move {
            while let Some(evt) = event_rx.recv().await {
                let mut stream_event = mapper.map(evt);

                // Derive timeline and metrics
                let timeline_entry = state.process_event(&stream_event);

                // Inject snapshot metrics into FinishedEvent if completing
                if let StreamEventPayload::Finished(ref mut f) = stream_event.payload {
                    if let Some(m) = state.metrics(execution_id) {
                        f.metrics = m;
                        f.metrics.duration_ms = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or(Duration::ZERO)
                            .as_millis() as u64; // Fallback or updated duration
                    }
                }

                // Broadcast main event
                state.broadcast(execution_id, stream_event.clone());

                // Broadcast timeline event if generated
                if let Some(entry) = timeline_entry {
                    let timeline_event = StreamEvent {
                        execution_id,
                        sequence: stream_event.sequence,
                        timestamp: stream_event.timestamp,
                        payload: StreamEventPayload::Timeline(TimelineEvent { entry }),
                    };
                    state.broadcast(execution_id, timeline_event);
                }
            }

            // Cleanup state
            if let Some(s) = states.write().remove(&execution_id) {
                s.cleanup(execution_id);
            }
        });
    }

    /// Subscribes a new event stream to the registered execution.
    pub fn subscribe(&self, execution_id: ExecutionId) -> Option<ExecutionStream> {
        let lock = self.states.read();
        lock.get(&execution_id)
            .and_then(|s| s.subscribe(execution_id))
    }

    /// Triggers cancellation for an execution.
    pub fn cancel(&self, execution_id: ExecutionId) -> bool {
        let lock = self.states.read();
        if let Some(state) = lock.get(&execution_id) {
            state.cancel();
            true
        } else {
            false
        }
    }

    /// Returns timeline snapshots.
    pub fn timeline(&self, execution_id: ExecutionId) -> Vec<TimelineEntry> {
        let lock = self.states.read();
        lock.get(&execution_id)
            .map(|s| s.timeline(execution_id))
            .unwrap_or_default()
    }
}
