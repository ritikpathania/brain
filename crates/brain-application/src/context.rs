use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Stable request identifier generated for tracing and logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ApplicationRequestId(pub Uuid);

impl ApplicationRequestId {
    /// Generate a new ApplicationRequestId.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ApplicationRequestId {
    fn default() -> Self {
        Self::new()
    }
}

/// A protocol-neutral representation of execution progress events.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ProgressEvent {
    /// Active index of progress step.
    pub step: u32,
    /// Total expected steps, if known.
    pub total_steps: Option<u32>,
    /// Informative log message.
    pub message: String,
    /// Semantic key-value attributes.
    pub metadata: std::collections::BTreeMap<String, String>,
}

/// Unified enum representing semantic application-level lifecycle and progress events.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum ApplicationEvent {
    /// Progress step completion signal.
    Progress(ProgressEvent),
    /// Non-fatal operational warnings.
    Warning(String),
    /// Technical diagnostic details.
    Diagnostic(String),
    /// Capability processing completed notification.
    Completed(String),
}

/// Abstract handler interface for routing ApplicationEvents to downstream adapters.
pub trait ApplicationEventSink: Send + Sync {
    /// Emit an ApplicationEvent.
    fn emit(&self, event: ApplicationEvent);
}

/// An execution context passed to capabilities. It coordinates request identities,
/// task cancellations, deadlines, and telemetry event sinks.
pub struct ExecutionContext {
    /// Correlation tracking request identity.
    pub request_id: ApplicationRequestId,
    /// Token indicating request cancellation state.
    pub cancellation_token: CancellationToken,
    /// Instant deadline threshold.
    pub deadline: Option<Instant>,
    /// Downstream telemetry emitter.
    pub event_sink: Option<Arc<dyn ApplicationEventSink>>,
}

impl ExecutionContext {
    /// Create a default empty ExecutionContext.
    pub fn new() -> Self {
        Self {
            request_id: ApplicationRequestId::new(),
            cancellation_token: CancellationToken::new(),
            deadline: None,
            event_sink: None,
        }
    }

    /// Add a request identifier to the context.
    pub fn with_request_id(mut self, request_id: ApplicationRequestId) -> Self {
        self.request_id = request_id;
        self
    }

    /// Add a cancellation token.
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = token;
        self
    }

    /// Add an execution deadline.
    pub fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Add a protocol-neutral event sink.
    pub fn with_event_sink<P: ApplicationEventSink + 'static>(mut self, sink: P) -> Self {
        self.event_sink = Some(Arc::new(sink));
        self
    }

    /// Emit an ApplicationEvent via the configured sink, if present.
    pub fn emit(&self, event: ApplicationEvent) {
        if let Some(sink) = &self.event_sink {
            sink.emit(event);
        }
    }

    /// Helper to emit progress updates.
    pub fn emit_progress(&self, step: u32, total_steps: Option<u32>, message: impl Into<String>) {
        self.emit(ApplicationEvent::Progress(ProgressEvent {
            step,
            total_steps,
            message: message.into(),
            metadata: std::collections::BTreeMap::new(),
        }));
    }

    /// Helper to emit warnings.
    pub fn emit_warning(&self, warning: impl Into<String>) {
        self.emit(ApplicationEvent::Warning(warning.into()));
    }

    /// Helper to emit diagnostics.
    pub fn emit_diagnostic(&self, diag: impl Into<String>) {
        self.emit(ApplicationEvent::Diagnostic(diag.into()));
    }

    /// Helper to emit completion notifications.
    pub fn emit_completed(&self, msg: impl Into<String>) {
        self.emit(ApplicationEvent::Completed(msg.into()));
    }

    /// Helper asserting cancellation token state.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}
